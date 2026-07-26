use crate::{Result, proto::MergableDataProto, proto::NoteStoreProto, utils};
use prost::Message;

pub fn parse_note_store_proto(data: &[u8]) -> Result<NoteStoreProto> {
    let decompressed = if utils::is_gzip(data) {
        utils::decompress_gzip(data)?
    } else {
        data.to_vec()
    };

    let proto = NoteStoreProto::decode(&decompressed[..])?;
    Ok(proto)
}

pub fn parse_mergable_data_proto(data: &[u8]) -> Result<MergableDataProto> {
    let decompressed = if utils::is_gzip(data) {
        utils::decompress_gzip(data)?
    } else {
        data.to_vec()
    };

    let proto = MergableDataProto::decode(&decompressed[..])?;
    Ok(proto)
}
