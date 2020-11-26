use anyhow::Result;
use ethercat_soem as soem;
use ethercat_types as ec;

pub fn main() -> Result<()> {
    env_logger::init();
    let args: Vec<_> = std::env::args().collect();
    if args.len() > 1 {
        read_od_list(&args[1])?;
    } else {
        println!("Usage: read_od_list <IFNAME>");
    }
    Ok(())
}

fn read_od_list(ifname: &str) -> Result<()> {
    let mut master = soem::Master::try_new(ifname)?;
    master.auto_config()?;

    let list = master.read_od_list(ec::SlavePos::from(0))?;
    println!("{:#?}", list);

    Ok(())
}
