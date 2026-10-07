//! 性能探针：拆分一次端口查询各阶段耗时，用于优化前后对比（默认不随 cargo test 运行）。
//! 运行：cargo test --release --test perf_probe -- --ignored --nocapture

use std::net::TcpListener;
use std::time::Instant;

use port_helper_lib::process::{self, services, ProcessCatalog};
use port_helper_lib::{net, query};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

fn median_ms(mut f: impl FnMut()) -> f64 {
    f(); // 预热
    let mut v: Vec<f64> = (0..15)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

#[test]
#[ignore = "性能基准，手动运行"]
fn probe() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let me = std::process::id();

    let rows = net::list_all().unwrap();
    let total_rows = rows.len();
    let matched = rows.iter().filter(|b| b.local_port == port).count();

    let t_net = median_ms(|| drop(net::list_all().unwrap()));
    let t_sys_all = median_ms(|| {
        let mut s = System::new();
        s.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    });
    let t_services = median_ms(|| drop(services::service_map(&[me])));
    let t_catalog = median_ms(|| drop(ProcessCatalog::load(&[me])));
    let t_start = median_ms(|| { std::hint::black_box(process::start_time(me).unwrap()); });
    let t_query = median_ms(|| {
        let all = net::list_all().unwrap();
        let catalog = ProcessCatalog::load(&[me]);
        drop(query::port_result(port, all, false, &catalog));
    });
    let t_guard = median_ms(|| {
        drop(process::image_name(me).unwrap());
        std::hint::black_box(process::start_time(me).unwrap());
    });

    let mut s = System::new();
    s.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let proc_count = s.processes().len();
    let svc_count: usize = services::service_map(&[me]).values().map(|v| v.len()).sum();

    println!("\n==== perf probe (median of 15) ====");
    println!("端口表总行数 {total_rows}，匹配 {matched}；进程数 {proc_count}；服务数 {svc_count}");
    println!("net::list_all             {t_net:>7.2} ms");
    println!("sysinfo 全量刷新(基础信息) {t_sys_all:>7.2} ms");
    println!("services::service_map     {t_services:>7.2} ms");
    println!("process::start_time       {t_start:>7.2} ms");
    println!("ProcessCatalog::load      {t_catalog:>7.2} ms");
    println!("query_port 全流程（含详情）{t_query:>7.2} ms");
    println!("操作前校验（名称+启动时间）{t_guard:>7.2} ms");
}
