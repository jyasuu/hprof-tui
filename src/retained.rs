//! Retained-size computation for the "Object Graph" tab.
//!
//! Algorithm:
//!  1. Parse the HPROF a second time, building a compact object graph:
//!     object_id → { class_id, shallow_size, outgoing_refs[] }
//!  2. Count how many objects reference each object (in-degree / ref_count).
//!  3. For each class: BFS from every instance, following only exclusively-owned
//!     edges (ref_count == 1). Accumulate retained bytes per child class → "ingredients".
//!
//! This gives:
//!  - Per-class retained_size (sum across all instances)
//!  - Per-class "ingredient" breakdown: how much byte-weight comes from char[],
//!    byte[], HashMap$Node, etc.
//!  - overhead_ratio = retained / shallow  (e.g. String ≈ 3-5× because of char[])

use anyhow::Result;
use jvm_hprof::{parse_hprof, IdSize, RecordTag};
use memmap2::MmapOptions;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RetainedClassEntry {
    pub class_name: String,
    pub instance_count: u64,
    pub shallow_size: u64,
    pub retained_size: u64,
    pub avg_retained: u64,
    pub overhead_ratio: f64, // retained / shallow
    pub top_contributors: Vec<Contributor>,
}

#[derive(Debug, Clone)]
pub struct Contributor {
    pub class_name: String,
    pub total_bytes: u64,
    pub object_count: u64,
}

pub struct RetainedAnalysis {
    pub entries: Vec<RetainedClassEntry>,
    pub truncated: bool, // true if we hit the object cap
}

// ── Main entry point ─────────────────────────────────────────────────────────

pub fn compute_retained(path: &str) -> Result<RetainedAnalysis> {
    let file = File::open(path)?;
    let mmap = unsafe { MmapOptions::new().map(&file) }?;
    let hprof = parse_hprof(&mmap[..]).map_err(|e| anyhow::anyhow!("parse: {:?}", e))?;
    let id_size = hprof.header().id_size();
    let id_bytes = match id_size {
        IdSize::U32 => 4usize,
        IdSize::U64 => 8usize,
    };

    // ── Pass 1: names ────────────────────────────────────────────────────────
    let mut utf8: HashMap<u64, String> = HashMap::new();
    let mut cmap: HashMap<u64, u64> = HashMap::new();

    for rec in hprof.records_iter().flatten() {
        match rec.tag() {
            RecordTag::Utf8 => {
                if let Some(Ok(u)) = rec.as_utf_8() {
                    utf8.insert(
                        u.name_id().id(),
                        String::from_utf8_lossy(u.text()).into_owned(),
                    );
                }
            }
            RecordTag::LoadClass => {
                if let Some(Ok(lc)) = rec.as_load_class() {
                    cmap.insert(lc.class_obj_id().id(), lc.class_name_id().id());
                }
            }
            _ => {}
        }
    }

    let resolve = |cid: u64| -> String {
        cmap.get(&cid)
            .and_then(|nid| utf8.get(nid))
            .map(|s| s.replace('/', "."))
            .unwrap_or_else(|| synthetic_name(cid))
    };

    // ── Pass 2: object graph ─────────────────────────────────────────────────
    // Keep memory under ~800 MB by capping at 5 M objects
    const CAP: usize = 5_000_000;

    // Compact storage: parallel vecs indexed by a local u32 index
    // obj_id → local index
    let mut id_to_idx: HashMap<u64, u32> = HashMap::with_capacity(CAP.min(1_000_000));
    let mut class_ids: Vec<u64> = Vec::new();
    let mut shallows: Vec<u64> = Vec::new();
    // edges stored as flat adjacency list: edge_start[i]..edge_start[i+1] → edge_targets
    let mut edge_start: Vec<u32> = Vec::new(); // len = objects+1
    let mut edge_targets: Vec<u32> = Vec::new();
    let mut ref_count: Vec<u32> = Vec::new(); // in-degree per object

    // Temp buffer: collect (from_idx, to_obj_id) then resolve after pass
    let mut pending_edges: Vec<(u32, u64)> = Vec::new();

    let mut truncated = false;

    'outer: for rec in hprof.records_iter().flatten() {
        if rec.tag() != RecordTag::HeapDump && rec.tag() != RecordTag::HeapDumpSegment {
            continue;
        }
        let seg = match rec.as_heap_dump_segment() {
            Some(Ok(s)) => s,
            _ => continue,
        };

        use jvm_hprof::heap_dump::SubRecord::*;
        for sub in seg.sub_records().flatten() {
            if id_to_idx.len() >= CAP {
                truncated = true;
                break 'outer;
            }

            match sub {
                Instance(inst) => {
                    let oid = inst.obj_id().id();
                    let cid = inst.class_obj_id().id();
                    let sz = inst.fields().len() as u64 + id_bytes as u64 * 2 + 4;
                    let idx = alloc_obj(
                        &mut id_to_idx,
                        &mut class_ids,
                        &mut shallows,
                        &mut ref_count,
                        oid,
                        cid,
                        sz,
                    );
                    // Scan field bytes for outgoing refs
                    for ref_id in scan_refs(inst.fields(), id_bytes) {
                        pending_edges.push((idx, ref_id));
                    }
                }
                PrimitiveArray(arr) => {
                    let oid = arr.obj_id().id();
                    let cid = primitive_array_cid(arr.primitive_type());
                    let sz = arr.contents().len() as u64 + id_bytes as u64 + 9;
                    alloc_obj(
                        &mut id_to_idx,
                        &mut class_ids,
                        &mut shallows,
                        &mut ref_count,
                        oid,
                        cid,
                        sz,
                    );
                    // primitive arrays have no outgoing object refs
                }
                ObjectArray(arr) => {
                    let oid = arr.obj_id().id();
                    let cid = arr.array_class_obj_id().id();
                    let sz = arr.num_elements() as u64 * id_bytes as u64 + id_bytes as u64 + 12;
                    alloc_obj(
                        &mut id_to_idx,
                        &mut class_ids,
                        &mut shallows,
                        &mut ref_count,
                        oid,
                        cid,
                        sz,
                    );
                    // object arrays: not traversing elements (conservative)
                }
                _ => {}
            }
        }
    }

    let n = id_to_idx.len();

    // Resolve pending edges → build CSR adjacency list + update ref_counts
    edge_start.resize(n + 1, 0u32);

    // First pass: count out-degree per node
    let mut out_deg: Vec<u32> = vec![0u32; n];
    let mut resolved_edges: Vec<(u32, u32)> = Vec::with_capacity(pending_edges.len() / 2);
    for (from_idx, to_oid) in &pending_edges {
        if let Some(&to_idx) = id_to_idx.get(to_oid) {
            out_deg[*from_idx as usize] += 1;
            ref_count[to_idx as usize] += 1;
            resolved_edges.push((*from_idx, to_idx));
        }
    }
    drop(pending_edges);

    // Build CSR
    edge_start[0] = 0;
    for i in 0..n {
        edge_start[i + 1] = edge_start[i] + out_deg[i];
    }
    edge_targets.resize(edge_start[n] as usize, 0u32);
    let mut fill = out_deg.clone();
    for i in 0..n {
        fill[i] = 0;
    }
    for (from, to) in resolved_edges {
        let pos = edge_start[from as usize] + fill[from as usize];
        edge_targets[pos as usize] = to;
        fill[from as usize] += 1;
    }
    drop(out_deg);
    drop(fill);

    // ── BFS retained-size computation per class ───────────────────────────────
    // Group object indices by class_id
    let mut by_class: HashMap<u64, Vec<u32>> = HashMap::new();
    for (oid, &idx) in &id_to_idx {
        by_class
            .entry(class_ids[idx as usize])
            .or_default()
            .push(idx);
    }

    // class_id → (retained_bytes, HashMap<child_class_id → (bytes, count)>)
    let mut retained_map: HashMap<u64, u64> = HashMap::new();
    let mut contrib_map: HashMap<u64, HashMap<u64, (u64, u64)>> = HashMap::new();
    let mut shallow_map: HashMap<u64, u64> = HashMap::new();

    for (&cid, instances) in &by_class {
        let mut total_retained = 0u64;
        let mut total_shallow = 0u64;
        let mut contribs: HashMap<u64, (u64, u64)> = HashMap::new();

        for &root_idx in instances {
            let root_sz = shallows[root_idx as usize];
            total_shallow += root_sz;
            total_retained += root_sz;
            let e = contribs
                .entry(class_ids[root_idx as usize])
                .or_insert((0, 0));
            e.0 += root_sz;
            e.1 += 1;

            // BFS: only follow exclusively-owned edges (ref_count == 1)
            let mut visited: HashSet<u32> = HashSet::new();
            let mut queue: VecDeque<u32> = VecDeque::new();
            visited.insert(root_idx);
            queue.push_back(root_idx);

            while let Some(cur) = queue.pop_front() {
                let start = edge_start[cur as usize] as usize;
                let end = edge_start[cur as usize + 1] as usize;
                for &child_idx in &edge_targets[start..end] {
                    if visited.contains(&child_idx) {
                        continue;
                    }
                    if ref_count[child_idx as usize] == 1 {
                        visited.insert(child_idx);
                        let child_sz = shallows[child_idx as usize];
                        let child_cid = class_ids[child_idx as usize];
                        total_retained += child_sz;
                        let ce = contribs.entry(child_cid).or_insert((0, 0));
                        ce.0 += child_sz;
                        ce.1 += 1;
                        queue.push_back(child_idx);
                    }
                }
            }
        }

        retained_map.insert(cid, total_retained);
        shallow_map.insert(cid, total_shallow);
        contrib_map.insert(cid, contribs);
    }

    // ── Assemble entries ─────────────────────────────────────────────────────
    let mut entries: Vec<RetainedClassEntry> = retained_map
        .iter()
        .map(|(&cid, &retained)| {
            let shallow = *shallow_map.get(&cid).unwrap_or(&0);
            let count = by_class.get(&cid).map(|v| v.len() as u64).unwrap_or(1);
            let ratio = if shallow > 0 {
                retained as f64 / shallow as f64
            } else {
                1.0
            };

            let mut top_contributors: Vec<Contributor> = contrib_map
                .get(&cid)
                .map(|m| {
                    let mut v: Vec<Contributor> = m
                        .iter()
                        .filter(|(&ccid, _)| ccid != cid)
                        .map(|(&ccid, &(bytes, cnt))| Contributor {
                            class_name: resolve(ccid),
                            total_bytes: bytes,
                            object_count: cnt,
                        })
                        .collect();
                    v.sort_by(|a, b| b.total_bytes.cmp(&a.total_bytes));
                    v.truncate(8);
                    v
                })
                .unwrap_or_default();

            RetainedClassEntry {
                class_name: resolve(cid),
                instance_count: count,
                shallow_size: shallow,
                retained_size: retained,
                avg_retained: if count > 0 { retained / count } else { 0 },
                overhead_ratio: ratio,
                top_contributors,
            }
        })
        .collect();

    entries.sort_by(|a, b| b.retained_size.cmp(&a.retained_size));

    Ok(RetainedAnalysis { entries, truncated })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn alloc_obj(
    id_to_idx: &mut HashMap<u64, u32>,
    class_ids: &mut Vec<u64>,
    shallows: &mut Vec<u64>,
    ref_count: &mut Vec<u32>,
    oid: u64,
    cid: u64,
    sz: u64,
) -> u32 {
    let idx = id_to_idx.len() as u32;
    id_to_idx.insert(oid, idx);
    class_ids.push(cid);
    shallows.push(sz);
    ref_count.push(0);
    idx
}

fn scan_refs(fields: &[u8], id_bytes: usize) -> Vec<u64> {
    let mut refs = Vec::new();
    let mut i = 0;
    while i + id_bytes <= fields.len() {
        let id: u64 = if id_bytes == 4 {
            u32::from_be_bytes(fields[i..i + 4].try_into().unwrap_or([0; 4])) as u64
        } else {
            u64::from_be_bytes(fields[i..i + 8].try_into().unwrap_or([0; 8]))
        };
        if id != 0 {
            refs.push(id);
        }
        i += id_bytes;
    }
    refs
}

fn primitive_array_cid(pt: jvm_hprof::heap_dump::PrimitiveArrayType) -> u64 {
    use jvm_hprof::heap_dump::PrimitiveArrayType::*;
    // stable synthetic IDs for primitive array types
    match pt {
        Boolean => 0xFFFF_FFFF_0000_0001,
        Char => 0xFFFF_FFFF_0000_0002,
        Float => 0xFFFF_FFFF_0000_0003,
        Double => 0xFFFF_FFFF_0000_0004,
        Byte => 0xFFFF_FFFF_0000_0005,
        Short => 0xFFFF_FFFF_0000_0006,
        Int => 0xFFFF_FFFF_0000_0007,
        Long => 0xFFFF_FFFF_0000_0008,
    }
}

fn synthetic_name(cid: u64) -> String {
    match cid {
        0xFFFF_FFFF_0000_0001 => "boolean[]".into(),
        0xFFFF_FFFF_0000_0002 => "char[]".into(),
        0xFFFF_FFFF_0000_0003 => "float[]".into(),
        0xFFFF_FFFF_0000_0004 => "double[]".into(),
        0xFFFF_FFFF_0000_0005 => "byte[]".into(),
        0xFFFF_FFFF_0000_0006 => "short[]".into(),
        0xFFFF_FFFF_0000_0007 => "int[]".into(),
        0xFFFF_FFFF_0000_0008 => "long[]".into(),
        _ => format!("?@{:#x}", cid),
    }
}
