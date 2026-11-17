use crate::error::{CoreError, CoreResult};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat,
};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tempfile::NamedTempFile;

const MAX_CAPTURE_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct RecordingStatus {
    pub active: bool,
    pub elapsed_sec: f64,
    pub level: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct RecordingCapture {
    pub slide_id: String,
    pub recorded_at: String,
    pub duration_sec: f64,
    pub wav_bytes: Vec<u8>,
    pub warning: Option<String>,
}

struct CaptureSession {
    slide_id: String,
    recorded_at: String,
    started_at: Instant,
    samples: Arc<Mutex<Vec<i16>>>,
    level: Arc<AtomicU32>,
    error: Arc<Mutex<Option<String>>>,
    sample_rate: u32,
    channels: u16,
    stream: Option<cpal::Stream>,
}

#[derive(Default)]
pub struct AudioRecordingService {
    session: Mutex<Option<CaptureSession>>,
}

impl AudioRecordingService {
    pub fn start(&self, slide_id: &str) -> CoreResult<RecordingStatus> {
        let mut active = self
            .session
            .lock()
            .map_err(|_| CoreError::Conflict("錄音狀態鎖已失效".to_string()))?;
        if active.is_some() {
            return Err(CoreError::Conflict("目前已有錄音進行中".to_string()));
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| CoreError::Conflict("找不到可用的麥克風".to_string()))?;
        let config = device
            .default_input_config()
            .map_err(|error| CoreError::Conflict(format!("無法讀取麥克風格式：{error}")))?;
        let format = config.sample_format();
        if !matches!(
            format,
            SampleFormat::F32
                | SampleFormat::F64
                | SampleFormat::I8
                | SampleFormat::I16
                | SampleFormat::I32
                | SampleFormat::U8
                | SampleFormat::U16
                | SampleFormat::U32
        ) {
            return Err(CoreError::Conflict(format!(
                "目前麥克風格式 {format:?} 尚未支援"
            )));
        }
        if config.channels() == 0 || config.channels() > 8 {
            return Err(CoreError::Conflict("麥克風聲道數不在支援範圍".to_string()));
        }

        let samples = Arc::new(Mutex::new(Vec::<i16>::new()));
        let callback_samples = samples.clone();
        let level = Arc::new(AtomicU32::new(0.0_f32.to_bits()));
        let callback_level = level.clone();
        let stream_error = Arc::new(Mutex::new(None));
        let callback_error = stream_error.clone();
        let data_error = callback_error.clone();
        let stream = device
            .build_input_stream_raw(
                config.config(),
                format,
                move |data, _| {
                    let converted = match data.sample_format() {
                        SampleFormat::F32 => data.as_slice::<f32>().map(|values| {
                            values
                                .iter()
                                .map(|value| float_to_i16(*value))
                                .collect::<Vec<_>>()
                        }),
                        SampleFormat::F64 => data.as_slice::<f64>().map(|values| {
                            values
                                .iter()
                                .map(|value| float_to_i16(*value as f32))
                                .collect::<Vec<_>>()
                        }),
                        SampleFormat::I8 => data.as_slice::<i8>().map(|values| {
                            values
                                .iter()
                                .map(|value| i16::from(*value) << 8)
                                .collect::<Vec<_>>()
                        }),
                        SampleFormat::I16 => data.as_slice::<i16>().map(|values| values.to_vec()),
                        SampleFormat::I32 => data.as_slice::<i32>().map(|values| {
                            values
                                .iter()
                                .map(|value| (value >> 16) as i16)
                                .collect::<Vec<_>>()
                        }),
                        SampleFormat::U8 => data.as_slice::<u8>().map(|values| {
                            values
                                .iter()
                                .map(|value| ((*value as i16) - 128) << 8)
                                .collect::<Vec<_>>()
                        }),
                        SampleFormat::U16 => data.as_slice::<u16>().map(|values| {
                            values
                                .iter()
                                .map(|value| ((*value as i32 - 32768) >> 0) as i16)
                                .collect::<Vec<_>>()
                        }),
                        SampleFormat::U32 => data.as_slice::<u32>().map(|values| {
                            values
                                .iter()
                                .map(|value| ((*value as i64 - 2_147_483_648_i64) >> 16) as i16)
                                .collect::<Vec<_>>()
                        }),
                        _ => None,
                    };
                    let Some(converted) = converted else {
                        return;
                    };
                    let peak = converted
                        .iter()
                        .map(|sample| sample.unsigned_abs())
                        .max()
                        .unwrap_or(0) as f32
                        / i16::MAX as f32;
                    callback_level.store(peak.min(1.0).to_bits(), Ordering::Relaxed);
                    if let Ok(mut samples) = callback_samples.lock() {
                        if samples
                            .len()
                            .saturating_add(converted.len())
                            .saturating_mul(2)
                            > MAX_CAPTURE_BYTES
                        {
                            if let Ok(mut error) = data_error.lock() {
                                *error =
                                    Some("錄音已達 512MB 上限，請停止並保存目前內容".to_string());
                            }
                            return;
                        }
                        samples.extend_from_slice(&converted);
                    }
                },
                move |error| {
                    if let Ok(mut message) = callback_error.lock() {
                        *message = Some(error.to_string());
                    }
                },
                Some(Duration::from_secs(2)),
            )
            .map_err(|error| CoreError::Conflict(format!("無法建立麥克風串流：{error}")))?;
        stream
            .play()
            .map_err(|error| CoreError::Conflict(format!("無法啟動麥克風：{error}")))?;
        *active = Some(CaptureSession {
            slide_id: slide_id.to_string(),
            recorded_at: recorded_at_now(),
            started_at: Instant::now(),
            samples,
            level,
            error: stream_error,
            sample_rate: config.sample_rate(),
            channels: config.channels(),
            stream: Some(stream),
        });
        Ok(RecordingStatus {
            active: true,
            elapsed_sec: 0.0,
            level: 0.0,
            error: None,
        })
    }

    pub fn status(&self) -> CoreResult<RecordingStatus> {
        let active = self
            .session
            .lock()
            .map_err(|_| CoreError::Conflict("錄音狀態鎖已失效".to_string()))?;
        Ok(match active.as_ref() {
            Some(session) => RecordingStatus {
                active: true,
                elapsed_sec: session.started_at.elapsed().as_secs_f64(),
                level: f32::from_bits(session.level.load(Ordering::Relaxed)),
                error: session.error.lock().ok().and_then(|error| error.clone()),
            },
            None => RecordingStatus {
                active: false,
                elapsed_sec: 0.0,
                level: 0.0,
                error: None,
            },
        })
    }

    pub fn is_recording(&self) -> bool {
        self.session
            .lock()
            .map(|session| session.is_some())
            .unwrap_or(false)
    }

    /// Finish the current slide segment and begin buffering the next slide
    /// without stopping the microphone stream.
    pub fn split_for_slide(&self, next_slide_id: &str) -> CoreResult<Option<RecordingCapture>> {
        let mut active = self
            .session
            .lock()
            .map_err(|_| CoreError::Conflict("錄音狀態鎖已失效".to_string()))?;
        let Some(session) = active.as_mut() else {
            return Ok(None);
        };
        if session.slide_id == next_slide_id {
            return Ok(None);
        }

        let samples = session.samples.clone();
        let mut samples = samples
            .lock()
            .map_err(|_| CoreError::Conflict("錄音緩衝區鎖已失效".to_string()))?;
        let segment_samples = std::mem::take(&mut *samples);
        drop(samples);

        let slide_id = std::mem::replace(&mut session.slide_id, next_slide_id.to_string());
        let recorded_at = std::mem::replace(&mut session.recorded_at, recorded_at_now());
        let duration_sec = session.started_at.elapsed().as_secs_f64();
        session.started_at = Instant::now();
        if segment_samples.is_empty() {
            return Ok(None);
        }
        let warning = session
            .error
            .lock()
            .ok()
            .and_then(|message| message.clone());
        let wav_bytes = encode_wav(&segment_samples, session.sample_rate, session.channels)?;
        Ok(Some(RecordingCapture {
            slide_id,
            recorded_at,
            duration_sec,
            wav_bytes,
            warning,
        }))
    }

    pub fn stop(&self) -> CoreResult<RecordingCapture> {
        let session = self
            .session
            .lock()
            .map_err(|_| CoreError::Conflict("錄音狀態鎖已失效".to_string()))?
            .take()
            .ok_or_else(|| CoreError::Conflict("目前沒有進行中的錄音".to_string()))?;
        let CaptureSession {
            slide_id,
            recorded_at,
            started_at,
            samples,
            error,
            stream,
            sample_rate,
            channels,
            ..
        } = session;
        drop(stream);
        let samples = samples
            .lock()
            .map_err(|_| CoreError::Conflict("錄音緩衝區鎖已失效".to_string()))?
            .clone();
        if samples.is_empty() {
            return Err(CoreError::Conflict("麥克風沒有收到音訊資料".to_string()));
        }
        let warning = error.lock().ok().and_then(|message| message.clone());
        let wav_bytes = encode_wav(&samples, sample_rate, channels)?;
        Ok(RecordingCapture {
            slide_id,
            recorded_at,
            duration_sec: started_at.elapsed().as_secs_f64(),
            wav_bytes,
            warning,
        })
    }
}

fn float_to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

fn recorded_at_now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
}

fn encode_wav(samples: &[i16], sample_rate: u32, channels: u16) -> CoreResult<Vec<u8>> {
    let temporary = NamedTempFile::new().map_err(CoreError::Io)?;
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(temporary.path(), spec)
        .map_err(|error| CoreError::Conflict(format!("建立 WAV 失敗：{error}")))?;
    for sample in samples {
        writer
            .write_sample(*sample)
            .map_err(|error| CoreError::Conflict(format!("寫入 WAV 失敗：{error}")))?;
    }
    writer
        .finalize()
        .map_err(|error| CoreError::Conflict(format!("完成 WAV 失敗：{error}")))?;
    std::fs::read(temporary.path()).map_err(CoreError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn encodes_capture_as_pcm_wav_using_device_format() {
        let bytes = encode_wav(&[0, 100, -100, 20], 44_100, 1).unwrap();
        let reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        assert_eq!(reader.spec().sample_rate, 44_100);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(reader.duration(), 4);
    }

    #[test]
    fn splitting_keeps_recording_active_and_separates_slide_audio() {
        let samples = Arc::new(Mutex::new(vec![10_i16, 20, 30]));
        let session = CaptureSession {
            slide_id: "slide-a".to_string(),
            recorded_at: "2026-11-11T20:00:00.000+08:00".to_string(),
            started_at: Instant::now(),
            samples: samples.clone(),
            level: Arc::new(AtomicU32::new(0.0_f32.to_bits())),
            error: Arc::new(Mutex::new(None)),
            sample_rate: 44_100,
            channels: 1,
            stream: None,
        };
        let service = AudioRecordingService {
            session: Mutex::new(Some(session)),
        };

        let first = service.split_for_slide("slide-b").unwrap().unwrap();
        assert_eq!(first.slide_id, "slide-a");
        assert_eq!(first.recorded_at, "2026-11-11T20:00:00.000+08:00");
        assert!(service.is_recording());
        {
            let mut buffer = samples.lock().unwrap();
            buffer.extend_from_slice(&[40, 50]);
        }
        let second = service.stop().unwrap();
        assert_eq!(second.slide_id, "slide-b");
        let first_reader = hound::WavReader::new(Cursor::new(first.wav_bytes)).unwrap();
        let second_reader = hound::WavReader::new(Cursor::new(second.wav_bytes)).unwrap();
        assert_eq!(
            first_reader
                .into_samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [10, 20, 30]
        );
        assert_eq!(
            second_reader
                .into_samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [40, 50]
        );
    }
}
