extern crate sdl2;

use std::fmt::Debug;

use sdl2::event::Event;

fn main() {
    let sdl = match sdl2::init() {
        Ok(sdl) => sdl,
        Err(msg) => panic!("Could not initialize SDL: {}", msg),
    };

    let video = match sdl.video() {
        Ok(vs) => vs,
        Err(msg) => panic!("Could not obtain video subsystem: {}", msg),
    };

    let window_builder = sdl2::video::WindowBuilder::new(&video, "Riapyx", 640, 400);
    let window = match window_builder.build() {
        Ok(window) => window,
        Err(msg) => panic!("Could not build window: {}", msg),
    };

    let mut event_pump = match sdl.event_pump() {
        Ok(ep) => ep,
        Err(msg) => panic!("Could not obtain event pump: {}", msg),
    };

    let mut run_emulator = true;

    while run_emulator {
        // handle sdl events
        let mut event_it = event_pump.poll_iter();
        for event in event_it {
            match event {
                Event::Quit {timestamp: _} => {
                    run_emulator = false
                },
                _ => {}
            }
        }
    }
}