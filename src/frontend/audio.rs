use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use cpal::traits::{DeviceTrait, StreamTrait};
use dasp::{Sample, Signal};
use nes_backend::util::AtomicF32;

pub struct ReceiverSignal<T: dasp::Frame> {
    receiver: Receiver<T>,
    previous_value: T,
}

impl<T: dasp::Frame> ReceiverSignal<T> {
    pub fn new(receiver: Receiver<T>) -> Self {
        Self {
            receiver,
            previous_value: T::EQUILIBRIUM,
        }
    }
}

impl<T: dasp::Frame> Signal for ReceiverSignal<T> {
    type Frame = T;

    fn next(&mut self) -> Self::Frame {
        if let Ok(frame) = self.receiver.try_recv() {
            self.previous_value = frame;
        }
        self.previous_value
    }
}

pub struct AudioRuntime {
    device: cpal::Device,
    stream: Option<Box<dyn StreamTrait>>,
    volume: Arc<AtomicF32>,
}

impl AudioRuntime {
    pub const DEFAULT_VOLUME: f32 = 1.0;

    pub fn new(device: cpal::Device) -> Self {
        Self {
            device,
            stream: None,
            volume: Arc::new(AtomicF32::new(Self::DEFAULT_VOLUME)),
        }
    }

    pub fn volume(&self) -> f32 {
        self.volume.load(Ordering::Relaxed)
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume.store(volume, Ordering::Relaxed);
    }

    pub fn connect<S>(&mut self, signal: S, sample_rate: f64)
    where
        S: Signal<Frame = f32> + Send + 'static,
    {
        let supported_config = self.device.default_output_config().unwrap();
        let config = supported_config.config();

        let converter = dasp::signal::interpolate::Converter::from_hz_to_hz(
            signal,
            dasp::interpolate::floor::Floor::new(0.0),
            sample_rate,
            config.sample_rate.0 as f64,
        );

        match supported_config.sample_format() {
            cpal::SampleFormat::F32 => self.connect_with_format::<f32, _>(config, converter),
            cpal::SampleFormat::I16 => self.connect_with_format::<i16, _>(config, converter),
            cpal::SampleFormat::U16 => self.connect_with_format::<u16, _>(config, converter),
            _ => unimplemented!(),
        }
    }

    fn connect_with_format<T, S>(&mut self, config: cpal::StreamConfig, mut signal: S)
    where
        T: cpal::FromSample<f32> + cpal::SizedSample,
        S: Signal<Frame = f32> + Send + 'static,
    {
        let channels = config.channels as usize;
        let volume = self.volume.clone();

        let stream = self.device.build_output_stream(
            &config,
            move |data: &mut [T], _| {
                let current_volume = volume.load(Ordering::Relaxed);
                for frame in data.chunks_mut(channels) {
                    let sample = (signal.next() * current_volume).to_sample();
                    for channel in frame {
                        *channel = sample;
                    }
                }
            },
            |error| {
                eprintln!("Warning: Problem while playing audio: {error}");
            },
            None,
        ).unwrap();

        stream.play().unwrap();

        self.stream = Some(Box::new(stream));
    }

    pub fn disconnect(&mut self) {
        if let Some(stream) = self.stream.take() {
            stream.pause().ok();
        }
    }
}

impl Drop for AudioRuntime {
    fn drop(&mut self) {
        self.disconnect();
    }
}
