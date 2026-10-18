use super::*;
use std::time::Duration;

const STAT: &str = "cpu  100 0 50 800 50 10 10 0 0 0\ncpu0 60 0 30 400 10 5 5 0 0 0\ncpu1 40 0 20 400 40 5 5 0 0 0\nintr 1 2 3\n";

#[test]
fn the_cpu_lines_are_read_the_machine_first() {
    let cpus = parse_cpus(STAT);
    assert_eq!(cpus.len(), 3);
    assert_eq!(
        cpus[0],
        Ticks {
            total: 1020,
            idle: 850,
            kernel: 70
        }
    );
    assert_eq!(
        cpus[2],
        Ticks {
            total: 510,
            idle: 440,
            kernel: 30
        }
    );
}

#[test]
fn busy_is_the_share_not_idle_and_the_kernel_part_of_it() {
    let was = Ticks {
        total: 1000,
        idle: 800,
        kernel: 50,
    };
    let is = Ticks {
        total: 1100,
        idle: 850,
        kernel: 60,
    };
    assert_eq!(
        busy(was, is),
        Some(Busy {
            used: 50.0,
            kernel: 10.0
        })
    );
    // No time passed: nothing to say.
    assert_eq!(busy(is, is), None);
}

#[test]
fn memory_in_use_is_what_is_not_available() {
    let text = "MemTotal:  2000 kB\nMemFree:  500 kB\nMemAvailable:  1500 kB\nBuffers: 10 kB\nCached: 900 kB\nSReclaimable: 90 kB\nShmem: 100 kB\nCommitted_AS: 700 kB\nCommitLimit: 1000 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n";
    let memory = parse_meminfo(text).unwrap();
    assert_eq!(memory.in_use(), 500 * 1024);
    assert_eq!(memory.cached, 1000 * 1024);
    assert_eq!(memory.committed, 700 * 1024);
    assert!(parse_meminfo("MemFree: 1 kB\n").is_none());
}

#[test]
fn disks_and_interfaces_are_read_from_their_counters() {
    let disks = parse_diskstats(" 254       0 vda 421 5 200 28 7 0 100 3 0 20 28 0 0 0 0 0 0\n");
    assert_eq!(
        disks,
        vec![(
            "vda".to_string(),
            DiskCount {
                read: 200 * 512,
                written: 100 * 512,
                busy_ms: 20
            }
        )]
    );
    let nets = parse_net_dev(
        "Inter-|   Receive                            |  Transmit\n face |bytes    packets errs drop fifo frame compressed multicast|bytes\n  eth0: 1820 15 0 0 0 0 0 0 18902 15 0 0 0 0 0 0\n",
    );
    assert_eq!(
        nets,
        vec![(
            "eth0".to_string(),
            NetCount {
                received: 1820,
                sent: 18902
            }
        )]
    );
    assert_eq!(
        parse_loadavg("0.13 0.09 0.10 1/124 15626\n"),
        Some(((0.13, 0.09, 0.10), 124))
    );
    let (model, mhz) = parse_cpuinfo(
        "model name\t: AMD Ryzen\ncpu MHz\t\t: 4000.0\nmodel name\t: AMD Ryzen\ncpu MHz\t\t: 5000.0\n",
    );
    assert_eq!(model.as_deref(), Some("AMD Ryzen"));
    assert_eq!(mhz, Some(4500.0));
}

#[test]
fn only_disks_a_person_would_call_one_are_shown() {
    assert!(is_disk("vda") && is_disk("sda") && is_disk("nvme0n1") && is_disk("sr0"));
    assert!(!is_disk("loop0") && !is_disk("dm-1") && !is_disk("fd0") && !is_disk("zram0"));
}

#[test]
fn speeds_are_the_difference_over_the_time_between_looks() {
    let at = Instant::now();
    let before = Counters {
        at,
        cpus: parse_cpus(STAT),
        disks: vec![(
            "vda".into(),
            DiskCount {
                read: 0,
                written: 0,
                busy_ms: 0,
            },
        )],
        nets: vec![(
            "eth0".into(),
            NetCount {
                received: 0,
                sent: 0,
            },
        )],
    };
    let after = Counters {
        at: at + Duration::from_secs(2),
        cpus: parse_cpus(STAT),
        disks: vec![(
            "vda".into(),
            DiskCount {
                read: 4096,
                written: 2048,
                busy_ms: 500,
            },
        )],
        nets: vec![(
            "eth0".into(),
            NetCount {
                received: 1000,
                sent: 250,
            },
        )],
    };
    let sample = between(&before, &after);
    assert_eq!(
        sample.disks,
        vec![(
            "vda".to_string(),
            DiskRate {
                active: 25.0,
                read: 2048.0,
                write: 1024.0
            }
        )]
    );
    assert_eq!(
        sample.nets,
        vec![(
            "eth0".to_string(),
            NetRate {
                received: 500.0,
                sent: 125.0
            }
        )]
    );
    // The same ticks twice: no time passed on the processors.
    assert_eq!(sample.cpu, None);
    assert_eq!(sample.cpus, vec![None, None]);
}

#[test]
fn a_graph_fills_from_the_right_and_leaves_gaps_where_nothing_was_read() {
    let svg = plot(
        &[Line {
            class: "k-cpu",
            values: vec![Some(50.0), None, Some(100.0)],
        }],
        100.0,
    );
    // Three looks of sixty: the newest at the right edge.
    assert!(
        svg.contains("<polyline class=\"k-cpu\" points=\"57.00,50.00 57.00,50.00\"/>"),
        "{svg}"
    );
    assert!(
        svg.contains("<polyline class=\"k-cpu\" points=\"59.00,0.00 59.00,0.00\"/>"),
        "{svg}"
    );
    // A run of two is filled beneath.
    let svg = plot(
        &[Line {
            class: "k-cpu",
            values: vec![Some(0.0), Some(100.0)],
        }],
        100.0,
    );
    assert!(svg.contains("<polygon class=\"fill k-cpu\" points=\"58.00,100.00 58.00,100.00 59.00,0.00 59.00,100.00\"/>"), "{svg}");
    // More than are kept: only the newest are drawn.
    let svg = plot(
        &[Line {
            class: "k-cpu",
            values: vec![Some(1.0); KEPT + 5],
        }],
        100.0,
    );
    assert!(
        svg.contains("<polyline class=\"k-cpu\" points=\"0.00,99.00 "),
        "{svg}"
    );
    assert!(
        !svg.contains(",-") && !svg.contains("\"-") && !svg.contains(" -"),
        "{svg}"
    );
}

#[test]
fn a_speed_graph_tops_out_at_a_round_figure() {
    assert_eq!(
        round_top(0.0, 1024.0, &BYTE_UNITS),
        (1024.0, "1 KB/s".into())
    );
    assert_eq!(
        round_top(3000.0, 1024.0, &BYTE_UNITS),
        (5120.0, "5 KB/s".into())
    );
    assert_eq!(round_top(1.5e6, 1000.0, &BIT_UNITS), (2e6, "2 Mbps".into()));
}

fn look(cpu: f64) -> Look {
    Look {
        sample: Sample {
            cpu: Some(Busy {
                used: cpu,
                kernel: 1.0,
            }),
            cpus: vec![Some(cpu), Some(cpu)],
            memory: Some(Memory {
                total: 2 << 30,
                available: 1 << 30,
                free: 1 << 29,
                ..Memory::default()
            }),
            disks: vec![(
                "vda".into(),
                DiskRate {
                    active: 3.0,
                    read: 2048.0,
                    write: 0.0,
                },
            )],
            nets: vec![(
                "eth0".into(),
                NetRate {
                    received: 1000.0,
                    sent: 125.0,
                },
            )],
        },
        about: About {
            model: Some("AMD Ryzen".into()),
            mhz: Some(4700.0),
            logical: 2,
            disks: vec![Disk {
                name: "vda".into(),
                kind: DiskKind::Disk,
                capacity: 1 << 30,
                model: None,
            }],
            nets: vec![Net {
                name: "eth0".into(),
                kind: NetKind::Ethernet,
                state: Some("up".into()),
                address: Some("52:54:00:12:34:56".into()),
                speed: None,
            }],
            ..About::default()
        },
        unread: Vec::new(),
    }
}

#[test]
fn every_resource_is_listed_and_the_cpu_shown_first() {
    let mut perf = Perf::default();
    perf.heard(look(12.0));
    let html = perf.render(30, Some(4000.0));
    assert!(
        html.contains("<b>CPU</b><small>12% · 4.70 GHz</small>"),
        "{html}"
    );
    assert!(
        html.contains("<b>Memory</b><small>1.0 GB of 2.0 GB (50%)</small>"),
        "{html}"
    );
    assert!(
        html.contains("<b>Disk (vda)</b><small>3.0% active</small>"),
        "{html}"
    );
    assert!(
        html.contains("<b>Ethernet (eth0)</b><small>Send 1.0 Kbps · Receive 8.0 Kbps</small>"),
        "{html}"
    );
    assert!(
        html.contains("fx-value-id=\"cpu\" aria-pressed=\"true\""),
        "{html}"
    );
    assert!(
        html.contains("<dt>Utilisation</dt><dd>12%, the kernel 1.0%</dd>"),
        "{html}"
    );
    assert!(html.contains("<dt>Processes</dt><dd>30</dd>"), "{html}");
    assert!(
        html.contains("<dt>Up for</dt><dd>1 hour, 6 minutes</dd>"),
        "{html}"
    );
}

#[test]
fn a_resource_picked_is_shown_in_full_until_it_goes() {
    let mut perf = Perf::default();
    perf.heard(look(12.0));
    assert!(perf.event("pick-resource", &serde_json::json!({ "id": "net:eth0" })));
    let html = perf.render(30, None);
    assert!(html.contains("<h2>Ethernet (eth0)</h2>"), "{html}");
    assert!(html.contains("<dt>State</dt><dd>Connected</dd>"), "{html}");
    assert!(
        html.contains("<dt>Hardware address</dt><dd>52:54:00:12:34:56</dd>"),
        "{html}"
    );
    // The interface gone, the CPU is shown again.
    let mut gone = look(12.0);
    gone.about.nets.clear();
    perf.heard(gone);
    assert!(perf.render(30, None).contains("<h2>CPU</h2>"));
    // Each processor, a graph of its own.
    perf.event("cpu-graph", &serde_json::json!({ "by": "each" }));
    let html = perf.render(30, None);
    assert!(html.contains("<span>CPU 1</span>"), "{html}");
}

#[test]
fn only_the_last_two_minutes_are_kept() {
    let mut perf = Perf::default();
    for n in 0..KEPT + 10 {
        perf.heard(look(n as f64));
    }
    assert_eq!(perf.history.len(), KEPT);
    assert_eq!(
        perf.history.front().and_then(|s| s.cpu).map(|b| b.used),
        Some(10.0)
    );
}
