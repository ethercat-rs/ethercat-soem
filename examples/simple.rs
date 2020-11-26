use anyhow::Result;
use ethercat_soem as soem;
use ethercat_types as ec;
use std::{thread, time::Duration};

pub fn main() -> Result<()> {
    env_logger::init();
    let args: Vec<_> = std::env::args().collect();
    if args.len() > 1 {
        simple(&args[1])?;
    } else {
        println!("Usage: simple_test <IFNAME>");
    }
    Ok(())
}

fn simple(ifname: &str) -> Result<()> {
    let mut master = soem::Master::try_new(ifname)?;
    master.auto_config()?;

    let list = master.read_od_list(ec::SlavePos::from(0))?;
    println!("{:#?}", list);

    log::debug!("wait for all slaves to reach SAFE_OP state");
    master.check_states(ec::AlState::SafeOp)?;

    let expected_wkc = master.group_outputs_wkc(0)? * 2 + master.group_inputs_wkc(0)?;
    log::debug!("Calculated workcounter {}", expected_wkc);

    log::debug!("Request operational state for all slaves");
    master.send_processdata()?;
    master.recv_processdata()?;
    master.request_states(ec::AlState::Op)?;

    for _ in 0..200 {
        master.send_processdata()?;
        master.recv_processdata()?;
        master.check_states(ec::AlState::Op)?;
        if master.states()?.iter().all(|s| *s == ec::AlState::Op) {
            log::info!("Operational state reached for all slaves");
            break;
        }
    }

    let cycle_time = Duration::from_micros(5_000);

    for i in 1..=5_000 {
        let cycle_start = std::time::Instant::now();
        master.send_processdata()?;
        let wkc = master.recv_processdata()?;
        if wkc >= expected_wkc {
            print!("Processdata cycle {}, WKC {}", i, wkc);
            println!(", T:{}", master.dc_time());
        }
        if let Some(dt) = cycle_time.checked_sub(cycle_start.elapsed()) {
            thread::sleep(dt);
        }
    }

    Ok(())
}
