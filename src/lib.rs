use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        CompactSize { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self.value {
            0..=0xFC => vec![self.value as u8],
            0xFD..=0xFFFF => {
                let mut vec = vec![0xFD];
                vec.extend_from_slice(&(self.value as u16).to_le_bytes());
                vec
            }
            0x10000..=0xFFFFFFFF => {
                let mut vec = vec![0xFE];
                vec.extend_from_slice(&(self.value as u32).to_le_bytes());
                vec
            }
            _ => {
                let mut vec = vec![0xFF];
                vec.extend_from_slice(&self.value.to_le_bytes());
                vec
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }

        let prefix = bytes[0];
        match prefix {
            0x00..=0xFC => Ok((CompactSize::new(prefix as u64), 1)),
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let val = u16::from_le_bytes(bytes[1..3].try_into().unwrap()) as u64;
                Ok((CompactSize::new(val), 3))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let val = u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as u64;
                Ok((CompactSize::new(val), 5))
            }
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let val = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
                Ok((CompactSize::new(val), 9))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex_str = String::deserialize(deserializer)?;
        let bytes = hex::decode(&hex_str).map_err(serde::de::Error::custom)?;
        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Invalid Txid length"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Txid(arr))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = self.txid.0.to_vec();
        result.extend_from_slice(&self.vout.to_le_bytes());
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[..32]);
        let vout = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
        Ok((OutPoint::new(txid, vout), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Script { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = CompactSize::new(self.bytes.len() as u64).to_bytes();
        result.extend_from_slice(&self.bytes);
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (size, size_len) = CompactSize::from_bytes(bytes)?;
        let total_len = size_len + size.value as usize;
        if bytes.len() < total_len {
            return Err(BitcoinError::InsufficientBytes);
        }
        let data = bytes[size_len..total_len].to_vec();
        Ok((Script::new(data), total_len))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        Self {
            previous_output,
            script_sig,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = self.previous_output.to_bytes();
        result.extend_from_slice(&self.script_sig.to_bytes());
        result.extend_from_slice(&self.sequence.to_le_bytes());
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (prev_out, offset1) = OutPoint::from_bytes(bytes)?;
        let (script, offset2) = Script::from_bytes(&bytes[offset1..])?;
        if bytes.len() < offset1 + offset2 + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let sequence = u32::from_le_bytes(
            bytes[offset1 + offset2..offset1 + offset2 + 4]
                .try_into()
                .unwrap(),
        );
        Ok((Self::new(prev_out, script, sequence), offset1 + offset2 + 4))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = self.version.to_le_bytes().to_vec();
        result.extend_from_slice(&CompactSize::new(self.inputs.len() as u64).to_bytes());
        for input in &self.inputs {
            result.extend_from_slice(&input.to_bytes());
        }
        result.extend_from_slice(&self.lock_time.to_le_bytes());
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let version = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let (size, offset1) = CompactSize::from_bytes(&bytes[4..])?;
        let mut inputs = Vec::new();
        let mut offset = 4 + offset1;

        for _ in 0..size.value {
            let (input, consumed) = TransactionInput::from_bytes(&bytes[offset..])?;
            inputs.push(input);
            offset += consumed;
        }

        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let lock_time = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        offset += 4;

        Ok((Self::new(version, inputs, lock_time), offset))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Inputs ({}):", self.inputs.len())?;
        for (i, input) in self.inputs.iter().enumerate() {
            writeln!(f, "  Input #{}:", i)?;
            writeln!(
                f,
                "    Previous Output TXID: {}",
                hex::encode(input.previous_output.txid.0)
            )?;
            writeln!(
                f,
                "    Previous Output Vout: {}",
                input.previous_output.vout
            )?;
            writeln!(
                f,
                "    ScriptSig ({} bytes): {}",
                input.script_sig.len(),
                hex::encode(&input.script_sig.bytes)
            )?;
            writeln!(f, "    Sequence: {}", input.sequence)?;
        }
        writeln!(f, "Lock Time: {}", self.lock_time)
    }
}
