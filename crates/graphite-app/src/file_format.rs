use graphite_i18n::Translator;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Encoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

impl Encoding {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
        }
    }
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Utf8 => Self::Utf8Bom,
            Self::Utf8Bom => Self::Utf16Le,
            Self::Utf16Le => Self::Utf16Be,
            Self::Utf16Be => Self::Utf8,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum LineEnding {
    #[default]
    Lf,
    Crlf,
}
impl LineEnding {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::Crlf => "CRLF",
        }
    }
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::Lf => Self::Crlf,
            Self::Crlf => Self::Lf,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DecodedFile {
    pub text: String,
    pub encoding: Encoding,
    pub line_ending: LineEnding,
}

pub(crate) fn read(path: &Path, translator: &Translator) -> Result<DecodedFile, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    decode(&bytes, translator)
}

pub(crate) fn decode(bytes: &[u8], translator: &Translator) -> Result<DecodedFile, String> {
    let (encoding, body) = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        (Encoding::Utf8Bom, &bytes[3..])
    } else if bytes.starts_with(&[0xff, 0xfe]) {
        (Encoding::Utf16Le, &bytes[2..])
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        (Encoding::Utf16Be, &bytes[2..])
    } else {
        (Encoding::Utf8, bytes)
    };
    let mut text = match encoding {
        Encoding::Utf8 | Encoding::Utf8Bom => std::str::from_utf8(body)
            .map(str::to_owned)
            .map_err(|_| translator.text("file.error.not_utf8"))?,
        Encoding::Utf16Le | Encoding::Utf16Be => {
            if body.len() % 2 != 0 {
                return Err(translator.text("file.error.incomplete_utf16").into());
            }
            let units = body
                .chunks_exact(2)
                .map(|pair| {
                    let pair = [pair[0], pair[1]];
                    if encoding == Encoding::Utf16Le {
                        u16::from_le_bytes(pair)
                    } else {
                        u16::from_be_bytes(pair)
                    }
                })
                .collect::<Vec<_>>();
            String::from_utf16(&units).map_err(|_| translator.text("file.error.invalid_utf16"))?
        }
    };
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    let line_ending = if crlf > 0 && crlf >= lf {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    };
    text = text.replace("\r\n", "\n").replace('\r', "\n");
    Ok(DecodedFile {
        text,
        encoding,
        line_ending,
    })
}

pub(crate) fn encode(text: &str, encoding: Encoding, line_ending: LineEnding) -> Vec<u8> {
    let normalized = text.replace("\r\n", "\n");
    let normalized = if line_ending == LineEnding::Crlf {
        normalized.replace('\n', "\r\n")
    } else {
        normalized
    };
    match encoding {
        Encoding::Utf8 => normalized.into_bytes(),
        Encoding::Utf8Bom => [b"\xef\xbb\xbf".as_slice(), normalized.as_bytes()].concat(),
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let mut bytes = if encoding == Encoding::Utf16Le {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in normalized.encode_utf16() {
                let pair = if encoding == Encoding::Utf16Le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                bytes.extend_from_slice(&pair);
            }
            bytes
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_and_roundtrips_supported_boms_unicode_and_eol() {
        let text = "olá 🌎\nlinha 2\n";
        for encoding in [
            Encoding::Utf8,
            Encoding::Utf8Bom,
            Encoding::Utf16Le,
            Encoding::Utf16Be,
        ] {
            for eol in [LineEnding::Lf, LineEnding::Crlf] {
                let bytes = encode(text, encoding, eol);
                let decoded = decode(&bytes, &Translator::english()).unwrap();
                assert_eq!(
                    decoded.text, text,
                    "encoding {encoding:?}, EOL {eol:?}, bytes {bytes:?}"
                );
                assert_eq!(decoded.encoding, encoding);
                assert_eq!(decoded.line_ending, eol);
            }
        }
    }

    #[test]
    fn rejects_malformed_utf8_and_odd_utf16() {
        let translator = Translator::english();
        assert!(decode(&[0xff], &translator).is_err());
        assert!(decode(&[0xff, 0xfe, 0x61], &translator).is_err());
    }
}
