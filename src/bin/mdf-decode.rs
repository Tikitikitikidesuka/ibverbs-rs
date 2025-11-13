use std::{
    io::{BufWriter, stdout},
    path::PathBuf,
};

use clap::{ColorChoice, Parser};
use colored::Colorize;
use master_data_file::MdfFile;
use pretty_hex::{HexConfig, config_hex};
use std::io::Write;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// File to decode
    file: PathBuf,
    #[arg(long)]
    /// Controls whether colored output is used
    color: ColorChoice,
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.color {
        ColorChoice::Auto => (),
        ColorChoice::Always => colored::control::set_override(true),
        ColorChoice::Never => colored::control::set_override(false),
    }

    let x = MdfFile::mmap_file(&args.file)?;

    let mut output = BufWriter::new(stdout());

    for rec in &x {
        write!(output, "{}", "MDF Record".bold().blue())?;
        if let Ok(rec) = rec.try_into_single_event() {
            writeln!(
                output,
                " {} {}, {} {}, {} {}",
                "Run".black(),
                rec.specific_header().run_number,
                "Orbit".black(),
                rec.specific_header().orbit_count,
                "Bunch".black(),
                rec.specific_header().bunch_identifier
            )?;

            for frag in rec.fragments() {
                let name = frag
                    .fragment_type_parsed()
                    .map(|ty| format!("{:?}", ty))
                    .unwrap_or_else(|| "Unknown".into());
                writeln!(
                    output,
                    "  {} {} ({:#X}) {} {}{} {}{} {} {}",
                    "Fragment".bold().black(),
                    name.green().bold(),
                    frag.fragment_type_raw(),
                    "Version".black(),
                    frag.version(),
                    ", Source".black(),
                    frag.source_id(),
                    ", Size".black(),
                    frag.fragment_size(),
                    "bytes".black()
                )?;

                if let Ok(odin) = frag.try_into_odin() {
                    let odin = odin.payload();

                    writeln!(
                        output,
                        "    {:<15} {1} ({1:#X})",
                        "Event Id".black(),
                        odin.event_id()
                    )?;
                    writeln!(
                        output,
                        "    {:<15} {:}",
                        "Event Type".black(),
                        odin.event_type()
                    )?;
                    writeln!(output, "    {:<15} {:}", "Time".black(), odin.gps_time())?;
                    writeln!(
                        output,
                        "    {:<15} {:#08X}",
                        "Partition".black(),
                        odin.partition_id()
                    )?;
                    writeln!(
                        output,
                        "    {:<15} {:}",
                        "Step enabled?".black(),
                        odin.step_run_enable()
                    )?;
                    if odin.step_run_enable() {
                        writeln!(
                            output,
                            "    {:<15} {} ({1:#X})",
                            "StepNumber".black(),
                            odin.step_number()
                        )?;
                    }
                    writeln!(
                        output,
                        "    {:<15} {:?} ({1:#X})",
                        "Orbit Id".black(),
                        odin.orbit_id()
                    )?;
                    writeln!(
                        output,
                        "    {:<15} {} ({1:#X})",
                        "Bunch Id".black(),
                        odin.bunch_id()
                    )?;
                    writeln!(
                        output,
                        "    {:<15} {:?}",
                        "BunchType".black(),
                        odin.bx_type()
                    )?;
                    writeln!(output, "    {:<15} {}", "TCK".black(), odin.tck())?;
                    writeln!(
                        output,
                        "    {:<15} {}",
                        "Is nzs event?".black(),
                        odin.is_nzs_event()
                    )?;
                    writeln!(
                        output,
                        "    {:<15} {} ({1:#X})",
                        "Calib type".black(),
                        odin.calib_type()
                    )?;
                    writeln!(
                        output,
                        "    {:<15} {:?} ({1:#X})",
                        "Trigger type".black(),
                        odin.trigger_type()
                    )?;
                    if odin.tae_window() > 0 {
                        writeln!(
                            output,
                            "    {:<15} {:?}",
                            "Tae window".black(),
                            odin.tae_window()
                        )?;
                        writeln!(
                            output,
                            "    {:<15} {:?}",
                            "Tae central".black(),
                            odin.tae_central()
                        )?;
                        writeln!(
                            output,
                            "    {:<15} {:?}",
                            "Tae first".black(),
                            odin.tae_first()
                        )?;
                    } else {
                        writeln!(output, "    {}", "Tae disabled".black())?;
                    }
                } else {
                    writeln!(
                        output,
                        "    {}",
                        config_hex(
                            &frag.payload_bytes(),
                            HexConfig {
                                title: false,
                                ascii: true,
                                width: 16,
                                group: 0,
                                chunk: 4,
                                max_bytes: 256,
                                display_offset: 0,
                            }
                        )
                        .replace("\n", "\n    ")
                    )?;
                }
            }
        } else {
            writeln!(output, " Header type {}", rec.specific_header_type())?;
        }
    }

    output.flush()?;
    Ok(())
}
