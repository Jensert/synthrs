use color_eyre::Result;
use core::f32;
use crossterm::event;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use ratatui::DefaultTerminal;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;
use ratatui::widgets::Block;
use ratatui::widgets::Gauge;
use ratatui::widgets::canvas::Canvas;
use ratatui::widgets::canvas::Line;
use rodio::OutputStreamHandle;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{OutputStream, Source};

mod widget;

#[derive(Clone, Copy)]
struct Vec2 {
    x: f64,
    y: f64,
}

struct WavetableOscillator {
    sample_rate: u32,
    wave_table: Arc<Vec<f32>>,
    index: f32,
    index_increment: f32,
    is_playing: bool,
}

impl WavetableOscillator {
    fn new(sample_rate: u32, wave_table: Arc<Vec<f32>>) -> Self {
        Self {
            sample_rate,
            wave_table,
            index: 0.0,
            index_increment: 0.0,
            is_playing: false,
        }
    }

    fn set_frequency(&mut self, frequency: f32) {
        self.index_increment = frequency * self.wave_table.len() as f32 / self.sample_rate as f32;
    }

    fn get_sample(&mut self) -> f32 {
        if !self.is_playing {
            return 0.0;
        }
        let sample = self.lerp();
        self.index += self.index_increment;
        self.index %= self.wave_table.len() as f32;
        sample
    }

    fn note_on(&mut self) {
        if self.is_playing {
            return;
        }
        self.is_playing = true;
        //println!("Playing");
    }

    fn note_off(&mut self) {
        if !self.is_playing {
            return;
        }
        self.is_playing = false;
        //println!("Stopping");
    }

    fn toggle(&mut self) {
        self.is_playing = !self.is_playing;
    }

    fn lerp(&self) -> f32 {
        let truncated_index = self.index as usize;
        let next_index = (truncated_index + 1) % self.wave_table.len();

        let next_index_weight = self.index - truncated_index as f32;
        let truncated_index_weight = 1.0 - next_index_weight;

        return truncated_index_weight * self.wave_table[truncated_index]
            + next_index_weight * self.wave_table[next_index];
    }
}

struct Voice {
    oscillator: WavetableOscillator,
    active: bool,
    key: Option<char>,
}
struct VoiceManager {
    voices: Vec<Voice>,
    sample_rate: u32,
    wave_table: Arc<Vec<f32>>,
}
impl VoiceManager {
    fn new(sample_rate: u32, wave_table: Arc<Vec<f32>>) -> Self {
        Self {
            voices: vec![],
            sample_rate,
            wave_table,
        }
    }

    fn note_on(&mut self, frequency: f32, key: char) {
        let active = self.voices.iter().any(|v| v.active && v.key == Some(key));
        if active {
            // Voice is already active, so skip it
            return;
        }
        let mut osc = WavetableOscillator::new(self.sample_rate, self.wave_table.clone());
        osc.set_frequency(frequency);
        osc.note_on();
        self.voices.push(Voice {
            oscillator: osc,
            active: true,
            key: Some(key),
        });
    }

    fn note_off(&mut self, key: char) {
        for voice in &mut self.voices {
            if voice.key == Some(key) {
                voice.active = false;
                voice.oscillator.note_off();
            }
        }
    }

    fn mix_sample(&mut self) -> f32 {
        let mut total = 0.0;
        let mut count = 0;
        self.voices.retain_mut(|voice| {
            if voice.active {
                total += voice.oscillator.get_sample();
                count += 1;
                true
            } else {
                false // drop inactive voices
            }
        });

        if count > 0 { total / count as f32 } else { 0.0 }
    }
}
struct StreamingSource {
    voice_manager: Arc<Mutex<VoiceManager>>,
    sample_rate: u32,
    master_volume: Arc<Mutex<f32>>,
}

impl Iterator for StreamingSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let volume = *self.master_volume.lock().unwrap();
        let sample = self.voice_manager.lock().unwrap().mix_sample();
        Some(sample * volume)
    }
}

impl Source for StreamingSource {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u16 {
        1
    }

    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub struct App {
    stream: OutputStream,
    stream_handle: OutputStreamHandle,
    wave_table_size: u16,
    sample_rate: u32,
    wave_table: Arc<Vec<f32>>,
    voice_manager: Arc<Mutex<VoiceManager>>,
    master_volume: Arc<Mutex<f32>>,

    terminal_size: Vec2,
    should_quit: bool,
    events: Vec<String>,
}
impl App {
    pub fn default() -> Self {
        let (stream, stream_handle) =
            OutputStream::try_default().expect("Expected a stream and stream handle");

        let wave_table_size = 256;
        let sample_rate = 44100;

        let wave_table: Arc<Vec<f32>> =
            Arc::new(generate_wave_table(Waveform::Sine, wave_table_size));
        let voice_manager = Arc::new(Mutex::new(VoiceManager::new(
            sample_rate,
            wave_table.clone(),
        )));

        let master_volume = Arc::new(Mutex::new(1.0));

        let src = StreamingSource {
            voice_manager: Arc::clone(&voice_manager),
            sample_rate,
            master_volume: master_volume.clone(),
        };

        stream_handle.play_raw(src.convert_samples()).unwrap();

        let events = Vec::new();

        Self {
            stream,
            stream_handle,
            wave_table_size,
            sample_rate,
            wave_table,
            voice_manager,
            master_volume,

            terminal_size: Vec2 { x: 10.0, y: 10.0 },
            should_quit: false,
            events,
        }
    }

    fn get_event(&self) -> Result<Option<Event>> {
        if event::poll(core::time::Duration::from_millis(10))? {
            // It's guaranteed that the `read()` won't block when the `poll()`
            // function returns `true`
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    fn process_event(&self, event: Option<Event>) -> Result<Action> {
        match event {
            Some(Event::Key(event)) => match event.code {
                KeyCode::Char('Q') => Ok(Action::Quit),
                KeyCode::Char('1') => Ok(Action::ChangeWavetable(Waveform::Sine)),
                KeyCode::Char('2') => Ok(Action::ChangeWavetable(Waveform::Saw)),
                KeyCode::Char('3') => Ok(Action::ChangeWavetable(Waveform::Square)),
                KeyCode::Char('4') => Ok(Action::ChangeWavetable(Waveform::Triangle)),
                KeyCode::Char('+') => Ok(Action::IncreaseVolume),
                KeyCode::Char('-') => Ok(Action::DecreaseVolume),

                KeyCode::Char(c) => {
                    let freq = match c {
                        'a' => Some(261.63),
                        's' => Some(293.66),
                        'd' => Some(329.63),
                        'f' => Some(349.23),
                        _ => None,
                    };

                    match event.kind {
                        event::KeyEventKind::Press => {
                            if let Some(freq) = freq {
                                Ok(Action::NoteOn(freq, c))
                            } else {
                                Ok(Action::None)
                            }
                        }
                        event::KeyEventKind::Release => {
                            if freq.is_some() {
                                Ok(Action::NoteOff(c))
                            } else {
                                Ok(Action::None)
                            }
                        }
                        _ => Ok(Action::None),
                    }
                }

                _ => Ok(Action::None),
            },

            Some(Event::Resize(x, y)) => Ok(Action::ChangeWindowSize(Vec2 {
                x: x as f64,
                y: y as f64,
            })),

            Some(Event::FocusGained) => Ok(Action::None),
            Some(Event::FocusLost) => Ok(Action::None),
            _ => Ok(Action::None),
        }
    }

    fn process_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::ChangeWindowSize(size) => self.terminal_size = size,
            Action::NoteOn(frequency, char) => {
                self.voice_manager.lock().unwrap().note_on(frequency, char)
            }
            Action::NoteOff(char) => self.voice_manager.lock().unwrap().note_off(char),
            Action::ChangeWavetable(waveform) => {
                let new_wavetable = Arc::new(generate_wave_table(waveform, self.wave_table_size));
                self.wave_table = Arc::clone(&new_wavetable);
                self.voice_manager.lock().unwrap().wave_table = Arc::clone(&new_wavetable);
            }
            Action::IncreaseVolume => {
                let mut volume = self.master_volume.lock().unwrap();
                if *volume < 1.0 {
                    *volume += 0.05
                }
            }
            Action::DecreaseVolume => {
                let mut volume = self.master_volume.lock().unwrap();
                if *volume > 0.05 {
                    *volume -= 0.05
                }
            }
            Action::None => (),

            _ => (),
        }
    }
}

enum Action {
    Quit,
    ChangeWindowSize(Vec2),
    NoteOn(f32, char),
    NoteOff(char),
    ChangeWavetable(Waveform),
    IncreaseVolume,
    DecreaseVolume,
    None,
}

enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

fn generate_wave_table(waveform: Waveform, size: u16) -> Vec<f32> {
    match waveform {
        Waveform::Sine => (0..size)
            .map(|n| (2.0 * std::f32::consts::PI * n as f32 / size as f32).sin())
            .collect(),
        Waveform::Saw => (0..size)
            .map(|n| 2.0 * (n as f32 / size as f32) - 1.0)
            .collect(),
        Waveform::Square => (0..size)
            .map(|n| if n < size / 2 { 1.0 } else { -1.0 })
            .collect(),
        Waveform::Triangle => (0..size)
            .map(|n| {
                let t = n as f32 / size as f32;
                4.0 * (t - 0.5).abs() - 1.0
            })
            .collect(),
    }
}

pub fn run(app: &mut App, mut terminal: DefaultTerminal) -> Result<()> {
    let bg_color = Color::Rgb(25, 50, 50);
    let fg_color = Color::Rgb(255, 123, 0);
    let accent_color = Color::Rgb(196, 255, 0);

    let mut phase = 0.0;
    loop {
        terminal.draw(|frame| {
            let screen_area = frame.area();
            let screen_block = Block::bordered()
                .title("Synthesizer")
                .style(Style::default().fg(fg_color).bg(bg_color));
            frame.render_widget(screen_block, screen_area);

            let waveform_area = Rect::new(
                (frame.area().width as f32 * 0.3) as u16,  // X
                (frame.area().height as f32 * 0.1) as u16, // Y
                (frame.area().width as f32 * 0.4) as u16,  // Width
                (frame.area().height as f32 * 0.4) as u16, // Height
            );

            phase += 0.05;
            if phase > 2.0 {
                phase -= 2.0
            }
            let resolution = app.wave_table.len();
            let data: Vec<(f64, f64)> = app
                .wave_table
                .iter()
                .enumerate()
                .map(|(i, &sample)| {
                    let mut x = (i as f64 / resolution as f64 * 2.0 - 1.0) + phase;
                    x = ((x + 1.0) % 2.0 + 2.0) % 2.0 - 1.0;
                    let y = sample as f64;
                    (x, y)
                })
                .collect();

            let canvas = Canvas::default()
                .block(Block::bordered().title("Wavetable"))
                .background_color(bg_color)
                .marker(Marker::HalfBlock)
                .x_bounds([-1.0, 1.0])
                .y_bounds([-1.0, 1.0])
                .paint(|ctx| {
                    for i in 1..data.len() {
                        let (x, y) = data[i];
                        ctx.draw(&ratatui::widgets::canvas::Points {
                            coords: &[(x, y)],
                            color: accent_color,
                        });
                    }
                });

            frame.render_widget(canvas, waveform_area);
            let volume = app.master_volume.lock().unwrap();
            let volume_gauge = Gauge::default()
                .block(Block::default().title("Volume"))
                .gauge_style(Style::default().fg(fg_color))
                .ratio(*volume as f64);
            let gauge_area = Rect::new(2, 2, 3, 10);
            frame.render_widget(volume_gauge, gauge_area);
        })?;

        let event = app.get_event()?;
        let action = app.process_event(event)?;
        app.process_action(action);

        if app.should_quit {
            break;
        }
    }
    Ok(())
}
