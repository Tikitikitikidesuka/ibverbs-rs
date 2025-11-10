use std::{
    io::{BufWriter, stdout},
    path::PathBuf,
};

use anyhow::Context;
use clap::Parser;
use master_data_file::MdfRecords;
use std::io::Write;
use time::UtcDateTime;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    file: PathBuf,
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let x = MdfRecords::mmap_file(&args.file)?;

    let mut output = BufWriter::new(stdout());

    for rec in &x {
        write!(output, "MDF Record:")?;
        if let Ok(rec) = rec.try_into_single_event() {
            writeln!(
                output,
                " Run {}, Orbit {}, Bunch {}",
                rec.specific_header().run_number,
                rec.specific_header().orbit_count,
                rec.specific_header().bunch_identifier
            )?;

            for frag in rec.fragments() {
                let name = frag
                    .fragment_type_parsed()
                    .map(|ty| format!("{:?}({})", ty, frag.fragment_type_raw()))
                    .unwrap_or_else(|| format!("UNKNOWN({:?})", frag.fragment_type_raw()));
                writeln!(
                    output,
                    "  Fragment {name} version {}: source {}, size {} bytes",
                    frag.version(),
                    frag.source_id(),
                    frag.size_bytes(),
                )?;
                if let Ok(odin) = frag.as_fragment().try_into_odin() {
                    let odin = odin.payload();
                    let time =
                        UtcDateTime::from_unix_timestamp_nanos(i128::from(odin.gps_time()) * 1_000)
                            .context("Convert Gps Time")?;
                    // writeln!(output, "    Time {:?}", odin.gps_time());
                    writeln!(output, "    Event Id {} ({0:#X})", odin.event_id())?;
                    writeln!(output, "    Event Type {:}", odin.event_type())?;
                    writeln!(output, "    Time {:}", time)?;
                    writeln!(output, "    Partition {:#08X}", odin.partition_id())?;
                    writeln!(output, "    Step enabled? {:}", odin.step_run_enable())?;
                    if odin.step_run_enable() {
                        writeln!(output, "    StepNumber {} ({0:#X})", odin.step_number())?;
                    }
                    writeln!(output, "    Orbit Id {:?} ({0:#X})", odin.orbit_id())?;
                    writeln!(output, "    Bunch Id {} ({0:#X})", odin.bunch_id())?;
                    writeln!(output, "    BunchType {:?}", odin.bx_type())?;
                    writeln!(output, "    TCK {}", odin.tck())?;
                    writeln!(output, "    is nzs event? {}", odin.is_nzs_event())?;
                    writeln!(output, "    calib type {} ({0:#X})", odin.calib_type())?;
                    writeln!(
                        output,
                        "    trigger type {:?} ({0:#X})",
                        odin.trigger_type()
                    )?;
                    if odin.tae_window() > 0 {
                        writeln!(output, "    tae window {:?}", odin.tae_window())?;
                        writeln!(output, "    tae central {:?}", odin.tae_central())?;
                        writeln!(output, "    tae first {:?}", odin.tae_first())?;
                    } else {
                        writeln!(output, "    tae disabled")?;
                    }
                }
            }
        } else {
            println!(" Header type {}", rec.specific_header_type());
        }
    }

    output.flush()?;
    Ok(())
}
