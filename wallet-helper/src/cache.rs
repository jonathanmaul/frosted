use async_trait::async_trait;
use std::{collections::BTreeMap, sync::Mutex};
use zcash_client_backend::{
    data_api::{
        chain::{BlockCache, BlockSource, error},
        scanning::ScanRange,
    },
    proto::compact_formats::CompactBlock,
};
use zcash_protocol::consensus::BlockHeight;
#[derive(Default)]
pub struct Cache(Mutex<BTreeMap<BlockHeight, CompactBlock>>);
impl BlockSource for Cache {
    type Error = std::io::Error;
    fn with_blocks<F, E>(
        &self,
        from: Option<BlockHeight>,
        limit: Option<usize>,
        mut f: F,
    ) -> Result<(), error::Error<E, Self::Error>>
    where
        F: FnMut(CompactBlock) -> Result<(), error::Error<E, Self::Error>>,
    {
        for (_, b) in self
            .0
            .lock()
            .unwrap()
            .range(from.unwrap_or(BlockHeight::from_u32(0))..)
            .take(limit.unwrap_or(usize::MAX))
        {
            f(b.clone())?
        }
        Ok(())
    }
}
#[async_trait]
impl BlockCache for Cache {
    fn get_tip_height(
        &self,
        range: Option<&ScanRange>,
    ) -> Result<Option<BlockHeight>, Self::Error> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .keys()
            .filter(|h| range.is_none_or(|r| r.block_range().contains(h)))
            .max()
            .copied())
    }
    async fn read(&self, range: &ScanRange) -> Result<Vec<CompactBlock>, Self::Error> {
        let blocks: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .range(range.block_range().clone())
            .map(|(_, b)| b.clone())
            .collect();
        for (i, b) in blocks.iter().enumerate() {
            if b.height != u64::from(range.block_range().start) + i as u64 {
                return Err(std::io::Error::other("noncontiguous block cache"));
            }
        }
        Ok(blocks)
    }
    async fn insert(&self, blocks: Vec<CompactBlock>) -> Result<(), Self::Error> {
        let mut cache = self.0.lock().unwrap();
        for b in blocks {
            let h = u32::try_from(b.height).map_err(std::io::Error::other)?;
            cache.insert(BlockHeight::from_u32(h), b);
        }
        Ok(())
    }
    async fn delete(&self, range: ScanRange) -> Result<(), Self::Error> {
        self.0
            .lock()
            .unwrap()
            .retain(|h, _| !range.block_range().contains(h));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zcash_client_backend::data_api::scanning::ScanPriority;
    #[tokio::test]
    async fn ordered_cache_rejects_gaps_and_truncates() {
        let c = Cache::default();
        let range = ScanRange::from_parts(
            BlockHeight::from_u32(1)..BlockHeight::from_u32(4),
            ScanPriority::Historic,
        );
        c.insert(vec![
            CompactBlock {
                height: 3,
                ..Default::default()
            },
            CompactBlock {
                height: 1,
                ..Default::default()
            },
        ])
        .await
        .unwrap();
        assert!(c.read(&range).await.is_err());
        c.insert(vec![CompactBlock {
            height: 2,
            ..Default::default()
        }])
        .await
        .unwrap();
        assert_eq!(
            c.read(&range)
                .await
                .unwrap()
                .iter()
                .map(|b| b.height)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        c.truncate(BlockHeight::from_u32(2)).await.unwrap();
        assert_eq!(
            c.get_tip_height(None).unwrap(),
            Some(BlockHeight::from_u32(2))
        );
    }
}
