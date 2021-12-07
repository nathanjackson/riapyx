extern crate ansi_term;
#[macro_use]
mod color_print;

mod cpu;
mod bios;
mod mem;
mod machine;
mod hw;

use std::io;
use std::io::prelude::*;
use std::string::String;
use std::collections::HashMap;
use std::env::args;
use std::{thread, time};
use std::sync::mpsc;
use std::sync::mpsc::{SyncSender, Receiver};
use bios::BootDrive;

extern crate rustc_serialize;
extern crate docopt;
use docopt::Docopt;

extern crate num_traits;
extern crate iced_x86;

const USAGE: &'static str = 
"Usage:
	riapyx [--boot=<drive>] [--hd=<image>] [--fd=<image>]
	riapyx [--help]

Options:
	--help           Display this message
	--hd=<img>       Hard drive image
	--fd=<img>       Floppy disk image
	--boot=<drive>   Boot from floppy disk (fd) or hard drive (hd) [default: hd]
";

#[derive(Debug, RustcDecodable)]
struct Args
{
	flag_hd: Option<String>,
	flag_fd: Option<String>,
	flag_boot: String
}

const ZERO_U8: u8 = '0' as u8;
const NINE_U8: u8 = '9' as u8;
const A_U8: u8 = 'a' as u8;
const F_U8: u8 = 'f' as u8;
fn u32_from_hex_str(s: &str) -> u32
{
	let mut res: u32 = 0;

	for c in s.as_bytes()
	{
		res *= 0x10;
		
		let to_add = 
			match *c
			{
				ZERO_U8 ... NINE_U8 => c - ZERO_U8,
				A_U8 ... F_U8 => (c - A_U8) + 0xa,
				_ => panic!("Invalid hex string: {}", s)
			};

		res += to_add as u32;
	}

	res
}

struct BreakpointManager
{
    next: i32,
    addr_bkpt: HashMap<(u16, u16), i32>,
}

impl BreakpointManager
{
    fn add_breakpoint(&mut self, seg: u16, addr: u16) -> i32
    {
        let bkpt = self.next;
        self.addr_bkpt.insert((seg, addr), bkpt);
        self.next += 1;
        bkpt
    }

    fn get_breakpoint(&self, seg: u16, addr: u16) -> Option<&i32>
    {
        self.addr_bkpt.get(&(seg, addr))
    }
}

trait Command
{
    fn execute(&self, m: &mut machine::Machine, bpm: &mut BreakpointManager);
}

struct QuitCommand { }

impl Command for QuitCommand
{
    fn execute(&self, m: &mut machine::Machine, bpm: &mut BreakpointManager)
    {
        std::process::exit(0);
    }
}

struct DumpCommand
{
    seg: u16,
    addr: u16
}

impl Command for DumpCommand
{
    fn execute(&self, m: &mut machine::Machine, bpm: &mut BreakpointManager)
    {
        m.print_memory(self.seg, self.addr, 16);
    }
}

struct ContinueCommand
{
    trace: bool
}

impl Command for ContinueCommand
{
    fn execute(&self, m: &mut machine::Machine, bpm: &mut BreakpointManager)
    {
        m.resume(self.trace);
    }
}

struct StepCommand { }

impl Command for StepCommand
{
    fn execute(&self, m: &mut machine::Machine, bpm: &mut BreakpointManager)
    {
        if !m.is_running() {
            m.resume(true);
            m.step();
            m.dump();
            m.pause();
        }
    }
}

struct InsertBreakpointCommand
{
    seg: u16,
    addr: u16
}

impl Command for InsertBreakpointCommand
{
    fn execute(&self, m: &mut machine::Machine, bpm: &mut BreakpointManager)
    {
        bpm.add_breakpoint(self.seg, self.addr);
    }
}

struct DisassembleCommand
{
    seg: u16,
    addr: u16
}

impl Command for DisassembleCommand
{
    fn execute(&self, m: &mut machine::Machine, bpm: &mut BreakpointManager)
    {
        m.disas(self.seg, self.addr, 5);
    }
}

struct WriteMemoryCommand
{
    filename: String,    
}

impl Command for WriteMemoryCommand
{
    fn execute(&self, m: &mut machine::Machine, bpm: &mut BreakpointManager)
    {
        m.dump_memory_to_file(&self.filename);
    }
}

fn console_thread(tx: SyncSender<Box<dyn Command + Send>>)
{
    loop {
        print!("debug> ");
        io::stdout().flush().unwrap();
        let mut cmd_str = String::new();
        io::stdin().read_line(&mut cmd_str).unwrap();

		let mut words = cmd_str.split_whitespace();

        let cmd: Box<dyn Command + Send>;
		let word0 = words.next();
		match word0
		{
            Some("q") => { 
                cmd = Box::new(QuitCommand{ });
            }
            Some("b") | Some("d") | Some("u") => {
				let args = (words.next(), words.next());
				match args
				{
					(Some(seg_str), Some(addr_str)) =>
					{
						let seg = u32_from_hex_str(seg_str) as u16;
						let addr = u32_from_hex_str(addr_str) as u16;

                        match word0 {
                            Some("b") => {
                                cmd = Box::new(InsertBreakpointCommand{
                                    seg,
                                    addr
                                });
                            }
                            Some("d") => {
                                cmd = Box::new(DumpCommand{
                                    seg,
                                    addr
                                });
                            }
                            Some("u") => {
                                cmd = Box::new(DisassembleCommand{
                                    seg,
                                    addr
                                });
                            }
                            _ => panic!("Impossible command!"),
                        }
					},
					_ => {
                        debug_print!("Usage: <CMD> [segment] [address]");
                        continue
                    }
                }
			}
            Some("c") | Some("t") => {
                let trace = Some("t") == word0;
                cmd = Box::new(ContinueCommand{
                    trace
                });
            }
			Some("w") =>
			{
				let arg = words.next();
				match arg
				{
					Some(fname) => {
                        cmd = Box::new(WriteMemoryCommand{
                            filename: fname.to_string()
                        });
					},
					_ => {
                        debug_print!("Usage: w [filename]");
                        continue
                    }				}
			}
            None => {
                cmd = Box::new(StepCommand{ });
            }
            _ => {
                println!("Bad command.");
                continue
            }
        }

        tx.send(cmd);
    }
}

fn main()
{
	let args: Args = Docopt::new(USAGE).and_then(|d| d.decode()).unwrap_or_else(|e| e.exit());

	let boot_drive = 
		match &args.flag_boot[..]
		{
			"fd" => BootDrive::Floppy,
			"hd" => BootDrive::HardDrive,
			_ => panic!("Unrecognized boot drive: '{}'. use 'hd' or 'fd'", args.flag_boot)
		};

	match boot_drive
	{
		BootDrive::Floppy =>
		{
			if let None = args.flag_fd
			{ panic!("Booting from floppy disk, but no floppy image specified"); }
		},
		BootDrive::HardDrive =>
		{
			if let None = args.flag_hd 
			{ panic!("Booting from hard drive, but no hard drive image specified"); }
		}
	}

	let mut m = machine::Machine::new(
				boot_drive,
				args.flag_fd,
				args.flag_hd);
	m.dump();

    // channel to communicate console commands to the emulator loop
    let (tx, rx): (SyncSender<Box<dyn Command + Send>>, Receiver<Box<dyn Command + Send>>) = mpsc::sync_channel(1);

    // console thread
    let console_thread_handle = thread::spawn(move || {
        console_thread(tx);
    });

    let mut bpm = BreakpointManager {
        next: 0,
        addr_bkpt: HashMap::new()
    };

    // main emulator loop
    loop {
        match rx.try_recv() {
            Ok(cmd) => {
                (*cmd).execute(&mut m, &mut bpm);
            }
            Err(_) => {
                m.step();

                if m.is_running() {
                    let (cs, ip) = m.get_pc();
                    match bpm.get_breakpoint(cs, ip) {
                        Some(bkpt) => {
                            debug_print!("Hit breakpoint #{} at {:04x}:{:04x}", bkpt, cs, ip);
                            m.pause();
                        }
                        _ => { }
                    }
                }
            }
        }
    }

    console_thread_handle.join().unwrap();
}
