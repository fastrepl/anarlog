use std::fs::File;
use std::io::{Cursor, Seek, SeekFrom};
use std::path::Path;

use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use memmap2::Mmap;

mod error;
pub use error::*;

mod template;
pub use template::*;

mod value;
pub use value::*;

mod utils;
pub use utils::*;

pub trait GgufExt {
    fn chat_format(&self) -> Result<Option<ChatTemplate>>;
    fn model_name(&self) -> Result<Option<String>>;
}

impl<T: AsRef<Path>> GgufExt for T {
    fn chat_format(&self) -> Result<Option<ChatTemplate>> {
        // First try to find explicit chat template
        if let Some(template) = read_gguf_metadata(
            self.as_ref(),
            |key, value_type, reader, version, is_little_endian| {
                if key == "tokenizer.chat_template" {
                    if let GGUFMetadataValueType::String = value_type {
                        let template = read_string(reader, version, is_little_endian)?;
                        return Ok(Some(ChatTemplate::TemplateValue(template)));
                    } else {
                        skip_value(reader, value_type, version, is_little_endian)?;
                    }
                } else {
                    skip_value(reader, value_type, version, is_little_endian)?;
                }
                Ok(None)
            },
        )? {
            return Ok(Some(template));
        }

        // If no explicit template, try to infer from architecture
        if let Some(architecture) = read_gguf_metadata(
            self.as_ref(),
            |key, value_type, reader, version, is_little_endian| {
                if key == "general.architecture" {
                    if let GGUFMetadataValueType::String = value_type {
                        let arch = read_string(reader, version, is_little_endian)?;
                        return Ok(Some(arch));
                    } else {
                        skip_value(reader, value_type, version, is_little_endian)?;
                    }
                } else {
                    skip_value(reader, value_type, version, is_little_endian)?;
                }
                Ok(None)
            },
        )? {
            match architecture.to_lowercase().as_str() {
                "llama" => Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::Llama2))),
                "mistral" => Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::MistralV1))),
                "falcon" => Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::Falcon3))),
                "mpt" => Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::ChatML))),
                "phi2" => Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::Phi3))),
                "gpt2" | "gptj" | "gptneox" => {
                    Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::ChatML)))
                }
                "llama3" => Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::Llama3))),
                "gemma" | "gemma3" => Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::Gemma))),
                "phi3" => Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::Phi3))),
                "phi4" => Ok(Some(ChatTemplate::TemplateKey(LlamaCppRegistry::Phi4))),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    fn model_name(&self) -> Result<Option<String>> {
        read_gguf_metadata(
            self.as_ref(),
            |key, value_type, reader, version, is_little_endian| {
                if key == "general.name" {
                    if let GGUFMetadataValueType::String = value_type {
                        let name = read_string(reader, version, is_little_endian)?;
                        return Ok(Some(name));
                    } else {
                        skip_value(reader, value_type, version, is_little_endian)?;
                    }
                } else {
                    skip_value(reader, value_type, version, is_little_endian)?;
                }
                Ok(None)
            },
        )
    }
}

fn read_gguf_metadata<F, R>(path: &Path, mut callback: F) -> Result<Option<R>>
where
    F: FnMut(&str, GGUFMetadataValueType, &mut Cursor<&[u8]>, u32, bool) -> Result<Option<R>>,
{
    let file = File::open(path)?;
    let map = unsafe { Mmap::map(&file)? };
    let mut reader = Cursor::new(&map[..]);

    let magic = reader.read_u32::<LittleEndian>()?;
    if magic != GGUF_MAGIC {
        return Err(Error::InvalidMagic);
    }

    let (version, is_little_endian) = {
        reader.seek(SeekFrom::Start(4))?;
        let version_le = reader.read_u32::<LittleEndian>()?;

        if version_le & 65535 != 0 {
            (version_le, true)
        } else {
            reader.seek(SeekFrom::Start(4))?;
            let version_be = reader.read_u32::<BigEndian>()?;
            (version_be, false)
        }
    };

    if version > 3 {
        return Err(Error::UnsupportedVersion(version));
    }

    // Reset position to after version
    reader.seek(SeekFrom::Start(8))?;

    let _tensor_count = read_versioned_size(&mut reader, version, is_little_endian)?;
    let metadata_kv_count = read_versioned_size(&mut reader, version, is_little_endian)?;

    for _ in 0..metadata_kv_count {
        let key = read_string(&mut reader, version, is_little_endian)?;

        let value_type_raw = if is_little_endian {
            reader.read_u32::<LittleEndian>()?
        } else {
            reader.read_u32::<BigEndian>()?
        };
        let value_type = GGUFMetadataValueType::try_from(value_type_raw)?;

        if let Some(result) = callback(&key, value_type, &mut reader, version, is_little_endian)? {
            return Ok(Some(result));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{ChatTemplate, Error, GgufExt, LlamaCppRegistry};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn write_string(buf: &mut Vec<u8>, s: &str) {
        buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
        buf.extend_from_slice(s.as_bytes());
    }

    fn write_kv_u32(buf: &mut Vec<u8>, key: &str, value: u32) {
        write_string(buf, key);
        buf.extend_from_slice(&4u32.to_le_bytes());
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn write_kv_string(buf: &mut Vec<u8>, key: &str, value: &str) {
        write_string(buf, key);
        buf.extend_from_slice(&8u32.to_le_bytes());
        write_string(buf, value);
    }

    fn write_gguf(kvs: impl FnOnce(&mut Vec<u8>), kv_count: u64) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gguf-test-{}-{}-{}.gguf",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            nanos
        ));

        let mut buf = Vec::new();
        buf.extend_from_slice(&0x46554747u32.to_le_bytes());
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes());
        buf.extend_from_slice(&kv_count.to_le_bytes());
        kvs(&mut buf);

        std::fs::File::create(&path)
            .unwrap()
            .write_all(&buf)
            .unwrap();
        path
    }

    #[test]
    fn reads_model_name_and_explicit_chat_template() {
        let path = write_gguf(
            |buf| {
                write_kv_u32(buf, "general.alignment", 32);
                write_kv_string(buf, "general.architecture", "llama");
                write_kv_string(buf, "general.name", "Test Model");
                write_kv_string(buf, "tokenizer.chat_template", "{{ messages }}");
            },
            4,
        );

        assert_eq!(path.model_name().unwrap(), Some("Test Model".to_string()));
        match path.chat_format().unwrap() {
            Some(ChatTemplate::TemplateValue(template)) => {
                assert_eq!(template, "{{ messages }}")
            }
            other => panic!("expected explicit template, got {other:?}"),
        }

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn infers_chat_template_from_architecture_and_rejects_bad_magic() {
        let path = write_gguf(
            |buf| {
                write_kv_string(buf, "general.architecture", "gemma3");
            },
            1,
        );

        assert_eq!(path.model_name().unwrap(), None);
        match path.chat_format().unwrap() {
            Some(ChatTemplate::TemplateKey(LlamaCppRegistry::Gemma)) => {}
            other => panic!("expected gemma template key, got {other:?}"),
        }
        std::fs::remove_file(&path).unwrap();

        let bad_path =
            std::env::temp_dir().join(format!("gguf-test-bad-{}.gguf", std::process::id()));
        std::fs::File::create(&bad_path)
            .unwrap()
            .write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x03, 0x00, 0x00, 0x00])
            .unwrap();
        assert!(matches!(bad_path.model_name(), Err(Error::InvalidMagic)));
        std::fs::remove_file(&bad_path).unwrap();
    }
}
