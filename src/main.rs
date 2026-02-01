use anyhow::{Context, Result};
use clap::Parser;
use crc::{Crc, CRC_32_ISO_HDLC};
use ihex::Reader;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(author, version, about = "Pico 2 W Rust Downloader")]
struct Args {
    #[arg(help = "Serial port (e.g. COM3 or /dev/ttyACM0)")]
    port: String,

    #[arg(help = "Path to .bin or .hex file")]
    file: PathBuf,

    #[arg(short, long, default_value_t = 115200, help = "Baud rate")]
    baud: u32,

    #[arg(short, long, default_value_t = 4096, help = "Chunk size")]
    chunk_size: usize,

    #[arg(short, long, help = "Send 'reboot' command before update")]
    reboot: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Load data
    let mut data = Vec::new();
    let extension = args
        .file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if extension == "hex" {
        println!("Parsing Intel HEX file: {:?}", args.file);
        let mut hex_content = String::new();
        File::open(&args.file)?.read_to_string(&mut hex_content)?;

        // Simple 2MB buffer for hex data
        let mut buffer = vec![0xFFu8; 2 * 1024 * 1024];
        let mut max_offset = 0;
        let mut base_addr = 0u32;

        let reader = Reader::new(&hex_content);
        for record in reader {
            let record = record.map_err(|e| anyhow::anyhow!("Hex parse error: {:?}", e))?;
            match record {
                ihex::Record::Data { offset, value } => {
                    let target_addr = base_addr + offset as u32;
                    // Map 0x10010100 -> 0 (relative binary for the app slot)
                    if target_addr >= 0x10010100 {
                        let rel_offset = (target_addr - 0x10010100) as usize;
                        if rel_offset + value.len() <= buffer.len() {
                            buffer[rel_offset..rel_offset + value.len()].copy_from_slice(&value);
                            max_offset = max_offset.max(rel_offset + value.len());
                        }
                    }
                }
                ihex::Record::ExtendedLinearAddress(upper) => {
                    base_addr = (upper as u32) << 16;
                }
                ihex::Record::EndOfFile => break,
                _ => {}
            }
        }
        if max_offset == 0 {
            anyhow::bail!("No data found in HEX file for address >= 0x10010100");
        }
        data = buffer[..max_offset].to_vec();
    } else {
        println!("Loading binary file: {:?}", args.file);
        File::open(&args.file)?.read_to_end(&mut data)?;

        // Safety Check: Detect if the user is using a file that already has metadata
        if data.len() >= 4 && &data[0..4] == b"APPS" {
            println!("WARNING: This file starts with 'APPS' magic. It appears to already have bootloader metadata.");
            println!("The downloader will calculate CRC over this file, which is likely NOT what you want.");
            println!("Expected: pico2w_shell.bin (Raw Binary)");
        }
    }

    if data.is_empty() {
        anyhow::bail!("Empty file or no valid data loaded.");
    }

    // 2. Calculate CRC32
    let crc_algo = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    let crc_val = crc_algo.checksum(&data);
    let len = data.len() as u32;

    println!("File loaded. Size: {} bytes, CRC32: 0x{:08X}", len, crc_val);

    // 3. Open Serial Port with robust settings
    let mut port = serialport::new(&args.port, args.baud)
        .timeout(Duration::from_millis(5000)) // 5s timeout for Windows stability
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .open()
        .with_context(|| format!("Failed to open port {}", args.port))?;

    // Some USB-serial adapters (and Windows drivers) require DTR/RTS to communicate properly
    port.write_data_terminal_ready(true).ok();
    port.write_request_to_send(true).ok();

    println!("Port {} opened at {} baud.", args.port, args.baud);

    // 4. Remote Reboot if requested
    if args.reboot {
        println!("Sending remote 'reboot' command...");
        port.write_all(b"reboot\r\n")?;
        port.flush()?;
        // Give more time for the Pico to reset and Boot ROM to run
        std::thread::sleep(Duration::from_millis(2000));
        port.clear(serialport::ClearBuffer::Input)?;
    }

    // 5. Enter Update Mode
    println!("Sending 'u' to trigger update mode...");
    port.write_all(b"u")?;
    port.flush()?;
    // Wait for the bootloader to process the trigger and enter wait_for_dfu
    std::thread::sleep(Duration::from_millis(1000));
    port.clear(serialport::ClearBuffer::Input)?;

    // 6. Send Magic Byte
    println!("Sending Magic 0xAA...");
    port.write_all(&[0xAA])?;

    // 7. Send Header
    println!("Sending Header: [Len={}, CRC=0x{:08X}]", len, crc_val);
    let mut header = [0u8; 8];
    header[0..4].copy_from_slice(&len.to_le_bytes());
    header[4..8].copy_from_slice(&crc_val.to_le_bytes());
    port.write_all(&header)?;

    // 8. Stream Data
    println!("Uploading data...");
    let pb = ProgressBar::new(len as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut sent = 0;
    while sent < data.len() {
        let end = (sent + args.chunk_size).min(data.len());
        port.write_all(&data[sent..end])?;
        sent = end;
        pb.set_position(sent as u64);
    }

    pb.finish_with_message("Upload complete!");
    println!("Done.");

    Ok(())
}
