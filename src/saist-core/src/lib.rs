pub mod app_container;
pub mod audio_storage;
pub mod deck_parser;
pub mod error;
pub mod manifest_repository;
pub mod master_export;
pub mod recording_service;
pub mod server;
pub mod slideshow_watcher;
pub mod types;
pub mod video_export;

pub use app_container::{AppContainer, SlideSyncHub};
pub use error::{CoreError, CoreResult};
pub use server::{router, serve};
pub use types::{
    ActiveSlideInfo, PresentationManifest, SlideTakeGroup, TakeCard, TakeGroupResponse,
};
