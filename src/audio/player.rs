use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

pub struct AudioPlayer {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sink: Sink,
    current_track: Option<String>,
    looping: bool,
}

impl AudioPlayer {
    /// Open the default audio output device.
    pub fn new() -> color_eyre::Result<Self> {
        let (_stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        Ok(Self {
            _stream,
            stream_handle,
            sink,
            current_track: None,
            looping: false,
        })
    }

    /// Play an audio file once. Stops any current playback.
    pub fn play(&mut self, path: &Path) -> color_eyre::Result<()> {
        self.sink.stop();
        self.sink = Sink::try_new(&self.stream_handle)?;
        let file = BufReader::new(File::open(path)?);
        let source = Decoder::new(file)?;
        self.sink.append(source);
        self.current_track = Some(track_name(path));
        self.looping = false;
        Ok(())
    }

    /// Play an audio file on infinite loop. Stops any current playback.
    pub fn play_loop(&mut self, path: &Path) -> color_eyre::Result<()> {
        self.sink.stop();
        self.sink = Sink::try_new(&self.stream_handle)?;
        let file = BufReader::new(File::open(path)?);
        let source = Decoder::new(file)?.repeat_infinite();
        self.sink.append(source);
        self.current_track = Some(track_name(path));
        self.looping = true;
        Ok(())
    }

    /// Stop all playback and clear the current track.
    pub fn stop(&mut self) {
        self.sink.stop();
        self.current_track = None;
        self.looping = false;
    }

    /// Pause current playback.
    pub fn pause(&self) {
        self.sink.pause();
    }

    /// Resume paused playback.
    pub fn resume(&self) {
        self.sink.play();
    }

    /// Set volume (clamped to 0.0..=1.0).
    pub fn set_volume(&self, vol: f32) {
        self.sink.set_volume(clamp_volume(vol));
    }

    /// Returns true if audio is currently playing (not paused, not empty).
    pub fn is_playing(&self) -> bool {
        !self.sink.empty() && !self.sink.is_paused()
    }

    /// Returns true if playback is paused.
    pub fn is_paused(&self) -> bool {
        self.sink.is_paused()
    }

    /// Returns the filename of the currently loaded track, if any.
    pub fn current_track(&self) -> Option<&str> {
        self.current_track.as_deref()
    }

    /// Returns true if the current track is looping.
    pub fn is_looping(&self) -> bool {
        self.looping
    }
}

/// Extract the filename from a path for display.
fn track_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Clamp volume to the valid range [0.0, 1.0].
fn clamp_volume(vol: f32) -> f32 {
    vol.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Volume clamping (no audio device needed) ---

    #[test]
    fn test_clamp_volume_within_range() {
        assert_eq!(clamp_volume(0.5), 0.5);
    }

    #[test]
    fn test_clamp_volume_at_zero() {
        assert_eq!(clamp_volume(0.0), 0.0);
    }

    #[test]
    fn test_clamp_volume_at_one() {
        assert_eq!(clamp_volume(1.0), 1.0);
    }

    #[test]
    fn test_clamp_volume_above_max() {
        assert_eq!(clamp_volume(2.5), 1.0);
    }

    #[test]
    fn test_clamp_volume_below_min() {
        assert_eq!(clamp_volume(-0.3), 0.0);
    }

    #[test]
    fn test_track_name_from_path() {
        let path = Path::new("/some/dir/tavern.mp3");
        assert_eq!(track_name(path), "tavern.mp3");
    }

    #[test]
    fn test_track_name_no_filename() {
        let path = Path::new("/");
        assert_eq!(track_name(path), "");
    }

    // --- AudioPlayer tests (need audio device — guarded with #[ignore]) ---

    #[test]
    #[ignore]
    fn test_audio_player_new() {
        let player = AudioPlayer::new();
        assert!(player.is_ok());
        let player = player.unwrap();
        assert!(!player.is_playing());
        assert!(!player.is_paused());
        assert!(player.current_track().is_none());
        assert!(!player.is_looping());
    }

    #[test]
    #[ignore]
    fn test_audio_player_stop_clears_track() {
        let mut player = AudioPlayer::new().unwrap();
        player.stop();
        assert!(player.current_track().is_none());
        assert!(!player.is_playing());
        assert!(!player.is_looping());
    }

    #[test]
    #[ignore]
    fn test_audio_player_pause_resume() {
        let player = AudioPlayer::new().unwrap();
        // Pause when nothing is playing — should not panic
        player.pause();
        assert!(player.is_paused());
        player.resume();
        assert!(!player.is_paused());
    }

    #[test]
    #[ignore]
    fn test_audio_player_set_volume() {
        let player = AudioPlayer::new().unwrap();
        // Volume setting should not panic with valid and clamped values
        player.set_volume(0.5);
        player.set_volume(0.0);
        player.set_volume(1.0);
        player.set_volume(-1.0); // clamped to 0.0
        player.set_volume(5.0); // clamped to 1.0
    }

    #[test]
    #[ignore]
    fn test_audio_player_play_missing_file() {
        let mut player = AudioPlayer::new().unwrap();
        let result = player.play(Path::new("/nonexistent/audio.wav"));
        assert!(result.is_err());
        assert!(player.current_track().is_none());
    }
}
