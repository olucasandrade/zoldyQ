use redis_protocol::resp2::types::OwnedFrame as RespFrame;

pub fn extract_string(frame: &RespFrame) -> Result<String, RespFrame> {
    match frame {
        RespFrame::BulkString(data) | RespFrame::SimpleString(data) => {
            Ok(String::from_utf8_lossy(data).to_string())
        }
        _ => Err(RespFrame::Error("ERR invalid string".to_string())),
    }
}

pub fn extract_bytes(frame: &RespFrame) -> Result<Vec<u8>, RespFrame> {
    match frame {
        RespFrame::BulkString(data) | RespFrame::SimpleString(data) => Ok(data.clone()),
        _ => Err(RespFrame::Error("ERR invalid bytes".to_string())),
    }
}

pub fn extract_integer(frame: &RespFrame) -> Result<i64, RespFrame> {
    match frame {
        RespFrame::Integer(n) => Ok(*n),
        RespFrame::BulkString(data) => {
            let s = String::from_utf8_lossy(&data);
            s.parse::<i64>()
                .map_err(|_| RespFrame::Error("ERR invalid integer".to_string()))
        }
        _ => Err(RespFrame::Error("ERR invalid integer".to_string())),
    }
}
