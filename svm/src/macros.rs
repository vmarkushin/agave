#[macro_export]
macro_rules! impl_borsh_serialize_deserialize {
    (borsh0_10, $ty:ty) => {
        impl borsh0_10::BorshSerialize for $ty {
            fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
                borsh::BorshSerialize::serialize(&self, writer)
            }
        }

        impl borsh0_10::BorshDeserialize for $ty {
            fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
                Ok(borsh::BorshDeserialize::deserialize_reader(reader)?)
            }
        }
    };
    ($borsh:ident, $ty:ty) => {
        impl $borsh::BorshSerialize for $ty {
            fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
                borsh::BorshSerialize::serialize(&self, writer)
            }
        }

        impl $borsh::BorshDeserialize for $ty {
            fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
                Ok(borsh::BorshDeserialize::deserialize(buf)?)
            }
        }
    };
}
