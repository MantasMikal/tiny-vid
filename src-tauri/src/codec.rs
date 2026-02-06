//! Codec metadata and build variant for FFmpeg.

use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodecInfo {
    pub value: String,
    pub name: String,
    pub formats: Vec<String>,
    pub supports_tune: bool,
    pub preset_type: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BuildVariantResult {
    pub variant: &'static str,
    pub codecs: Vec<CodecInfo>,
}

struct CodecRow {
    value: &'static str,
    name: &'static str,
    formats: &'static [&'static str],
    supports_tune: bool,
    preset_type: &'static str,
}

macro_rules! codec_table {
    (
        $( [$value:expr, $name:expr, $formats:expr, $tune:expr, $preset:expr] ),* $(,)?
    ) => {
        const CODEC_TABLE: &[CodecRow] = &[
            $( CodecRow {
                value: $value,
                name: $name,
                formats: $formats,
                supports_tune: $tune,
                preset_type: $preset,
            } ),*
        ];

        /// Supported codec names from CODEC_TABLE.
        pub const SUPPORTED_CODEC_NAMES: &[&str] = &[ $($value),* ];
    };
}

codec_table!(
    ["libx264", "H.264 (Widest support)", &["mp4", "mkv"], true, "x264"],
    ["libx265", "H.265 (Smaller files)", &["mp4", "mkv"], false, "x265"],
    ["libsvtav1", "AV1 (Smallest files)", &["mp4", "webm", "mkv"], false, "av1"],
    ["libvpx-vp9", "VP9 (Browser-friendly WebM)", &["webm", "mkv"], false, "vp9"],
    ["h264_videotoolbox", "H.264 (VideoToolbox)", &["mp4", "mkv"], false, "vt"],
    ["hevc_videotoolbox", "H.265 (VideoToolbox)", &["mp4", "mkv"], false, "vt"],
);

/// Return CodecInfo for a known codec string. Panics on unknown codec.
pub fn get_codec_info(codec: &str) -> CodecInfo {
    let row = CODEC_TABLE
        .iter()
        .find(|r| r.value == codec)
        .unwrap_or_else(|| panic!("Unknown codec: {}", codec));
    CodecInfo {
        value: row.value.to_string(),
        name: row.name.to_string(),
        formats: row.formats.iter().copied().map(str::to_string).collect(),
        supports_tune: row.supports_tune,
        preset_type: row.preset_type.to_string(),
    }
}

const NON_VT: &[&str] = &["libx264", "libx265", "libsvtav1", "libvpx-vp9"];
const VT: &[&str] = &["h264_videotoolbox", "hevc_videotoolbox"];

/// When non-LGPL (software) codecs are available, filter out VideoToolbox so we prefer libx264/etc.
pub fn filter_codecs_for_display(available: &[String]) -> Vec<String> {
    let has_non_vt = available.iter().any(|c| NON_VT.contains(&c.as_str()));
    if has_non_vt {
        available
            .iter()
            .filter(|c| !VT.contains(&c.as_str()))
            .cloned()
            .collect()
    } else {
        available.to_vec()
    }
}

pub fn get_build_variant(available: Vec<String>) -> Result<BuildVariantResult, AppError> {
    let codecs = filter_codecs_for_display(&available);

    if codecs.is_empty() {
        return Err(AppError::from(
            "No supported video codecs found in FFmpeg. Please ensure FFmpeg is properly installed with codec support."
        ));
    }

    #[cfg(feature = "lgpl")]
    let variant = "lgpl";
    #[cfg(not(feature = "lgpl"))]
    let variant = "standalone";

    Ok(BuildVariantResult {
        variant,
        codecs: codecs.iter().map(|s| get_codec_info(s)).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{filter_codecs_for_display, get_codec_info, CODEC_TABLE, SUPPORTED_CODEC_NAMES};

    #[test]
    fn codec_info_has_correct_metadata() {
        let info = get_codec_info("libx264");
        assert_eq!(info.value, "libx264");
        assert_eq!(info.name, "H.264 (Widest support)");
        assert_eq!(info.formats, vec!["mp4", "mkv"]);
        assert!(info.supports_tune);
        assert_eq!(info.preset_type, "x264");
    }

    #[test]
    fn all_codecs_have_info() {
        for codec in [
            "libx264",
            "libx265",
            "libsvtav1",
            "libvpx-vp9",
            "h264_videotoolbox",
            "hevc_videotoolbox",
        ] {
            let info = get_codec_info(codec);
            assert!(!info.value.is_empty());
            assert!(!info.name.is_empty());
        }
    }

    #[test]
    fn get_codec_info_returns_correct_formats() {
        let x264 = get_codec_info("libx264");
        assert_eq!(x264.formats, vec!["mp4", "mkv"]);

        let av1 = get_codec_info("libsvtav1");
        assert_eq!(av1.formats, vec!["mp4", "webm", "mkv"]);

        let vp9 = get_codec_info("libvpx-vp9");
        assert_eq!(vp9.formats, vec!["webm", "mkv"]);
    }

    #[test]
    fn get_codec_info_preset_types() {
        assert_eq!(get_codec_info("libx264").preset_type, "x264");
        assert_eq!(get_codec_info("libx265").preset_type, "x265");
        assert_eq!(get_codec_info("libsvtav1").preset_type, "av1");
        assert_eq!(get_codec_info("h264_videotoolbox").preset_type, "vt");
    }

    #[test]
    fn filter_codecs_hides_videotoolbox_when_non_vt_available() {
        let available = vec![
            "libx264".to_string(),
            "h264_videotoolbox".to_string(),
            "hevc_videotoolbox".to_string(),
        ];
        let filtered = filter_codecs_for_display(&available);
        assert_eq!(filtered, vec!["libx264"]);
    }

    #[test]
    fn codec_table_matches_supported_codec_names() {
        let table_names: Vec<&str> = CODEC_TABLE.iter().map(|r| r.value).collect();
        assert_eq!(
            table_names.len(),
            SUPPORTED_CODEC_NAMES.len(),
            "CODEC_TABLE and SUPPORTED_CODEC_NAMES should have same length"
        );
        for name in SUPPORTED_CODEC_NAMES {
            assert!(
                table_names.contains(name),
                "SUPPORTED_CODEC_NAMES includes {} but CODEC_TABLE does not",
                name
            );
        }
    }

    #[test]
    fn filter_codecs_keeps_videotoolbox_when_only_vt_available() {
        let available = vec![
            "h264_videotoolbox".to_string(),
            "hevc_videotoolbox".to_string(),
        ];
        let filtered = filter_codecs_for_display(&available);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"h264_videotoolbox".to_string()));
        assert!(filtered.contains(&"hevc_videotoolbox".to_string()));
    }
}
