use anyhow::{Context, Result};
use jvm_hprof::{parse_hprof, RecordTag};
use memmap2::MmapOptions;
use std::collections::HashMap;
use std::fs::File;

#[derive(Debug, Clone, Default)]
pub struct HeapSummary {
    pub file_path: String,
    pub file_size_bytes: u64,
    pub hprof_version: String,
    pub total_instances: u64,
    pub total_classes: u64,
    pub total_arrays: u64,
    pub total_gc_roots: u64,
    pub total_heap_size: u64,
}

#[derive(Debug, Clone)]
pub struct ClassEntry {
    pub class_name: String,
    pub instance_count: u64,
    pub shallow_size: u64,
}

#[derive(Debug, Clone)]
pub struct GcRoot {
    pub object_id: u64,
    pub root_type: String,
}

#[derive(Debug, Clone)]
pub struct DuplicateString {
    pub value_preview: String,
    pub count: u64,
    pub wasted_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct LeakSuspect {
    pub class_name: String,
    pub instance_count: u64,
    pub total_shallow_size: u64,
    pub heap_percentage: f64,
    pub severity: SuspectSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SuspectSeverity {
    High,
    Medium,
    Low,
}

impl SuspectSeverity {
    pub fn label(&self) -> &str {
        match self {
            Self::High => "HIGH",
            Self::Medium => "MED",
            Self::Low => "LOW",
        }
    }
}

#[derive(Debug)]
pub struct HprofAnalysis {
    pub summary: HeapSummary,
    pub class_histogram: Vec<ClassEntry>,
    pub gc_roots: Vec<GcRoot>,
    pub duplicate_strings: Vec<DuplicateString>,
    pub leak_suspects: Vec<LeakSuspect>,
}

pub fn analyze_hprof(path: &str) -> Result<HprofAnalysis> {
    let file = File::open(path).with_context(|| format!("Cannot open '{}'", path))?;
    let file_size_bytes = file.metadata()?.len();
    let mmap = unsafe { MmapOptions::new().map(&file) }.context("Failed to mmap HPROF file")?;

    let hprof =
        parse_hprof(&mmap[..]).map_err(|e| anyhow::anyhow!("HPROF parse failed: {:?}", e))?;

    let id_size = hprof.header().id_size();
    let hprof_version = hprof
        .header()
        .label()
        .unwrap_or("unknown")
        .trim_end_matches('\0')
        .to_string();

    // Pass 1: UTF-8 strings + class name map
    let mut utf8_map: HashMap<u64, String> = HashMap::new();
    let mut cls_name: HashMap<u64, u64> = HashMap::new();

    for rec in hprof.records_iter().flatten() {
        match rec.tag() {
            RecordTag::Utf8 => {
                if let Some(Ok(u)) = rec.as_utf_8() {
                    utf8_map.insert(
                        u.name_id().id(),
                        String::from_utf8_lossy(u.text()).into_owned(),
                    );
                }
            }
            RecordTag::LoadClass => {
                if let Some(Ok(lc)) = rec.as_load_class() {
                    cls_name.insert(lc.class_obj_id().id(), lc.class_name_id().id());
                }
            }
            _ => {}
        }
    }

    // Pass 2: heap segments
    let mut inst_count: HashMap<u64, u64> = HashMap::new();
    let mut inst_size: HashMap<u64, u64> = HashMap::new();
    let mut gc_roots: Vec<GcRoot> = Vec::new();
    let mut str_fps: HashMap<Vec<u8>, u64> = HashMap::new();

    let mut total_heap_size = 0u64;
    let mut total_gc_roots = 0u64;
    let mut total_arrays = 0u64;
    let mut total_instances = 0u64;

    let id_bytes = match id_size {
        jvm_hprof::IdSize::U32 => 4u64,
        jvm_hprof::IdSize::U64 => 8u64,
    };

    for rec in hprof.records_iter().flatten() {
        if rec.tag() != RecordTag::HeapDump && rec.tag() != RecordTag::HeapDumpSegment {
            continue;
        }
        let seg = match rec.as_heap_dump_segment() {
            Some(Ok(s)) => s,
            _ => continue,
        };

        use jvm_hprof::heap_dump::SubRecord::*;
        for sub in seg.sub_records().flatten() {
            match sub {
                GcRootUnknown(r) => {
                    gc_roots.push(GcRoot {
                        object_id: r.obj_id().id(),
                        root_type: "Unknown".into(),
                    });
                    total_gc_roots += 1;
                }
                GcRootThreadObj(r) => {
                    gc_roots.push(GcRoot {
                        object_id: r.thread_obj_id().map(|i| i.id()).unwrap_or(0),
                        root_type: "ThreadObj".into(),
                    });
                    total_gc_roots += 1;
                }
                GcRootJniGlobal(r) => {
                    gc_roots.push(GcRoot {
                        object_id: r.obj_id().id(),
                        root_type: "JniGlobal".into(),
                    });
                    total_gc_roots += 1;
                }
                GcRootJniLocalRef(r) => {
                    gc_roots.push(GcRoot {
                        object_id: r.obj_id().id(),
                        root_type: "JniLocalRef".into(),
                    });
                    total_gc_roots += 1;
                }
                GcRootJavaStackFrame(r) => {
                    gc_roots.push(GcRoot {
                        object_id: r.obj_id().id(),
                        root_type: "StackFrame".into(),
                    });
                    total_gc_roots += 1;
                }
                GcRootNativeStack(r) => {
                    gc_roots.push(GcRoot {
                        object_id: r.obj_id().id(),
                        root_type: "NativeStack".into(),
                    });
                    total_gc_roots += 1;
                }
                GcRootSystemClass(r) => {
                    gc_roots.push(GcRoot {
                        object_id: r.obj_id().id(),
                        root_type: "SystemClass".into(),
                    });
                    total_gc_roots += 1;
                }
                GcRootThreadBlock(r) => {
                    gc_roots.push(GcRoot {
                        object_id: r.obj_id().id(),
                        root_type: "ThreadBlock".into(),
                    });
                    total_gc_roots += 1;
                }
                GcRootBusyMonitor(r) => {
                    gc_roots.push(GcRoot {
                        object_id: r.obj_id().id(),
                        root_type: "BusyMonitor".into(),
                    });
                    total_gc_roots += 1;
                }
                Instance(inst) => {
                    let cid = inst.class_obj_id().id();
                    *inst_count.entry(cid).or_insert(0) += 1;
                    let sz = inst.fields().len() as u64 + id_bytes * 2 + 4;
                    *inst_size.entry(cid).or_insert(0) += sz;
                    total_heap_size += sz;
                    total_instances += 1;
                    // Fingerprint Strings for dup detection
                    if resolve_name(cid, &cls_name, &utf8_map) == "java.lang.String"
                        && !inst.fields().is_empty()
                    {
                        let key = inst.fields()[..inst.fields().len().min(24)].to_vec();
                        *str_fps.entry(key).or_insert(0) += 1;
                    }
                }
                ObjectArray(arr) => {
                    total_heap_size += arr.num_elements() as u64 * id_bytes + id_bytes + 4 + 4 + 4;
                    total_arrays += 1;
                }
                PrimitiveArray(arr) => {
                    total_heap_size += arr.contents().len() as u64 + id_bytes + 4 + 4 + 1;
                    total_arrays += 1;
                }
                _ => {}
            }
        }
    }

    let total_classes = cls_name.len() as u64;

    // Class histogram
    let mut histogram: Vec<ClassEntry> = inst_count
        .iter()
        .map(|(&cid, &count)| ClassEntry {
            class_name: resolve_name(cid, &cls_name, &utf8_map),
            instance_count: count,
            shallow_size: *inst_size.get(&cid).unwrap_or(&0),
        })
        .collect();
    histogram.sort_by(|a, b| b.shallow_size.cmp(&a.shallow_size));

    // Leak suspects >5%
    let heap_nz = total_heap_size.max(1);
    let mut leak_suspects: Vec<LeakSuspect> = histogram
        .iter()
        .filter_map(|e| {
            let pct = e.shallow_size as f64 / heap_nz as f64 * 100.0;
            if pct < 5.0 {
                return None;
            }
            let severity = if pct >= 30.0 {
                SuspectSeverity::High
            } else if pct >= 15.0 {
                SuspectSeverity::Medium
            } else {
                SuspectSeverity::Low
            };
            Some(LeakSuspect {
                class_name: e.class_name.clone(),
                instance_count: e.instance_count,
                total_shallow_size: e.shallow_size,
                heap_percentage: pct,
                severity,
            })
        })
        .collect();
    leak_suspects.sort_by(|a, b| b.heap_percentage.partial_cmp(&a.heap_percentage).unwrap());

    // Duplicate strings
    let mut duplicate_strings: Vec<DuplicateString> = str_fps
        .into_iter()
        .filter(|(_, c)| *c > 1)
        .map(|(k, count)| DuplicateString {
            value_preview: format!(
                "fp:{}",
                k.iter()
                    .take(6)
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            ),
            count,
            wasted_bytes: (count - 1) * 40,
        })
        .collect();
    duplicate_strings.sort_by(|a, b| b.wasted_bytes.cmp(&a.wasted_bytes));
    duplicate_strings.truncate(200);
    gc_roots.truncate(2000);

    Ok(HprofAnalysis {
        summary: HeapSummary {
            file_path: path.to_string(),
            file_size_bytes,
            hprof_version,
            total_instances,
            total_classes,
            total_arrays,
            total_gc_roots,
            total_heap_size,
        },
        class_histogram: histogram,
        gc_roots,
        duplicate_strings,
        leak_suspects,
    })
}

fn resolve_name(cid: u64, cls_name: &HashMap<u64, u64>, utf8: &HashMap<u64, String>) -> String {
    cls_name
        .get(&cid)
        .and_then(|nid| utf8.get(nid))
        .map(|s| s.replace('/', "."))
        .unwrap_or_else(|| format!("unknown@{:#x}", cid))
}

pub fn fmt_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    const U: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let i = ((bytes as f64).log(1024f64).floor() as usize).min(U.len() - 1);
    let v = bytes as f64 / 1024f64.powi(i as i32);
    if i > 1 {
        format!("{:.2} {}", v, U[i])
    } else {
        format!("{:.0} {}", v, U[i])
    }
}
