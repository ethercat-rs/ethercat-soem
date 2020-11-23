use anyhow::Result;
use ethercat_soem as soem;
use ethercat_types as ec;
use std::{thread, time::Duration};

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
    let mut master = soem::Master::new(ifname)?;
    master.auto_config()?;

    log::debug!("Request operational state for all slaves");
    master.request_states(ec::AlState::Op)?;

    let cycle_time = Duration::from_micros(5_000);
    let sdo_complete = false;
    let sdo_read_timeout = Duration::from_micros(700_000);

    let mut read_times = vec![];

    for _ in 0..=1000 {
        master.send_processdata()?;
        master.recv_processdata()?;
        let sdo_read_start = std::time::Instant::now();

        let data = master.read_sdo::<u32>(
            ec::SlavePos::from(0),
            ec::SdoIdx {
                idx: ec::Idx::from(0x1018),
                sub_idx: ec::SubIdx::from(0x01),
            },
            sdo_complete,
            sdo_read_timeout,
        )?;
        let dt = sdo_read_start.elapsed();
        println!("SDO read: {}ms, data: {}", dt.as_millis(), data);
        read_times.push(dt);
        if let Some(dt) = cycle_time.checked_sub(dt) {
            thread::sleep(dt);
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
