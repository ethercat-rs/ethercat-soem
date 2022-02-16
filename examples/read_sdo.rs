use anyhow::Result;
use ethercat_soem as soem;
use ethercat_types as ec;
use std::{
    thread,
    time::{Duration, Instant},
};

pub fn main() -> Result<()> {
    env_logger::init();
    let args: Vec<_> = std::env::args().collect();
    if args.len() > 1 {
        read_sdo(&args[1])?;
    } else {
        println!("Usage: read_sdo <IFNAME>");
    }
    Ok(())
}

fn read_sdo(ifname: &str) -> Result<()> {
    let mut master = soem::Master::try_new(ifname)?;
    master.auto_config()?;

    for s in master.slaves() {
        log::info!("Found {:#?}", s);
    }

    log::debug!("Request operational state for all slaves");
    master.request_states(ec::AlState::Op)?;
    master.check_states(ec::AlState::Op, Duration::from_millis(500))?;

    let cycle_time = Duration::from_micros(5_000);
    let mut read_times = vec![];

    // Dirty hack to use the same underlying SOEM context
    // within another thread.
    // usize works independent of the CPU architecture (32/64-bit)
    let master_ptr = master.ptr();
    let master_ptr_addr = master_ptr as usize;

    thread::spawn(move || {
        let mut rt_master = unsafe {
            let master_ptr = std::mem::transmute(master_ptr_addr);
            soem::Master::from_ptr(master_ptr)
        };
        loop {
            let cycle_start = Instant::now();
            rt_master.send_processdata().unwrap();
            rt_master.recv_processdata().unwrap();
            for s in rt_master.slaves() {
                log::debug!("Inputs:{:?}", s.inputs());
                log::debug!("Outputs:{:?}", s.outputs());
            }
            let dt = cycle_start.elapsed();
            log::debug!("Slave states: {:?}", rt_master.states());
            match cycle_time.checked_sub(dt) {
                Some(x) => {
                    log::debug!("Send & Receive took {}µs", x.as_micros());
                    thread::sleep(x);
                }
                None => {
                    log::warn!("Send & Receive took {}µs", dt.as_micros());
                }
            }
        }
    });

    let sdo_idxs = [4120, 4337, 32916];
    let sdo_read_timeout = Duration::from_millis(5_000);

    for _ in 0..=1000 {
        for idx in &sdo_idxs {
            let sdo_read_start = std::time::Instant::now();

            let data = master.read_sdo_complete(
                ec::SlavePos::from(0),
                ec::Idx::from(*idx),
                sdo_read_timeout,
            )?;
            let dt = sdo_read_start.elapsed();
            log::debug!("SDO read: {}ms, data: {:?}", dt.as_millis(), data);
            read_times.push(dt);
        }
    }

    let max = read_times.iter().max().unwrap();
    let min = read_times.iter().min().unwrap();
    let sum: Duration = read_times.iter().sum();
    let avg = sum.as_millis() / read_times.len() as u128;

    println!(
        "Max: {}ms, Min: {}ms, Avg: {}ms",
        max.as_millis(),
        min.as_millis(),
        avg
    );

    Ok(())
}
