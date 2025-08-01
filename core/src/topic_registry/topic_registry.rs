//! Topic registry implementation for Ferrite broker
//!
//! This module provides the topic management system with support for multiple
//! partitions per topic, as specified in issue #4.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::hash::{Hash, Hasher};

/// Unique identifier for a topic
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TopicId(String);

impl TopicId {
    pub fn new(name: String) -> Self {
        TopicId(name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Unique identifier for a partition within a topic
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartitionId(u32);

impl PartitionId {
    pub fn new(id: u32) -> Self {
        PartitionId(id)
    }

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

/// Represents a topic with its configuration
#[derive(Debug, Clone)]
pub struct Topic {
    id: TopicId,
    partition_count: u32,
    partitions: Vec<PartitionId>,
}

impl Topic {
    pub fn new(id: TopicId, partition_count: u32) -> Self {
        let partitions: Vec<PartitionId> = (0..partition_count)
            .map(PartitionId::new)
            .collect();
        
        Topic {
            id,
            partition_count,
            partitions,
        }
    }

    pub fn id(&self) -> &TopicId {
        &self.id
    }

    pub fn partition_count(&self) -> u32 {
        self.partition_count
    }

    pub fn partitions(&self) -> &[PartitionId] {
        &self.partitions
    }
}

/// Thread-safe topic registry for managing topics and their partitions
#[derive(Debug, Clone)]
pub struct TopicRegistry {
    /// Internal storage for topics
    topics: Arc<RwLock<HashMap<TopicId, Topic>>>,
}

impl TopicRegistry {
    /// Creates a new, empty topic registry
    pub fn new() -> Self {
        TopicRegistry {
            topics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a new topic with the specified number of partitions
    ///
    /// # Arguments
    ///
    /// * `topic_id` - The unique identifier for the topic
    /// * `partition_count` - The number of partitions for this topic
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Topic was successfully created
    /// * `Err(TopicRegistryError)` - If the topic already exists
    pub fn create_topic(&self, topic_id: TopicId, partition_count: u32) -> Result<(), TopicRegistryError> {
        let mut topics = self.topics.write().map_err(|_| TopicRegistryError::LockPoisoned)?;
        
        if topics.contains_key(&topic_id) {
            return Err(TopicRegistryError::TopicAlreadyExists(topic_id));
        }

        let topic = Topic::new(topic_id.clone(), partition_count);
        topics.insert(topic_id, topic);
        Ok(())
    }

    /// Deletes a topic and all its partitions
    ///
    /// # Arguments
    ///
    /// * `topic_id` - The unique identifier for the topic to delete
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Topic was successfully deleted
    /// * `Err(TopicRegistryError)` - If the topic does not exist
    pub fn delete_topic(&self, topic_id: &TopicId) -> Result<(), TopicRegistryError> {
        let mut topics = self.topics.write().map_err(|_| TopicRegistryError::LockPoisoned)?;
        
        if topics.remove(topic_id).is_none() {
            return Err(TopicRegistryError::TopicNotFound(topic_id.clone()));
        }
        
        Ok(())
    }

    /// Gets a topic by its identifier
    ///
    /// # Arguments
    ///
    /// * `topic_id` - The unique identifier for the topic
    ///
    /// # Returns
    ///
    /// * `Some(Topic)` - The topic if it exists
    /// * `None` - If the topic does not exist
    pub fn get_topic(&self, topic_id: &TopicId) -> Result<Option<Topic>, TopicRegistryError> {
        let topics = self.topics.read().map_err(|_| TopicRegistryError::LockPoisoned)?;
        Ok(topics.get(topic_id).cloned())
    }

    /// Lists all topics in the registry
    ///
    /// # Returns
    ///
    /// A vector containing all topics
    pub fn list_topics(&self) -> Result<Vec<Topic>, TopicRegistryError> {
        let topics = self.topics.read().map_err(|_| TopicRegistryError::LockPoisoned)?;
        Ok(topics.values().cloned().collect())
    }

    /// Gets the partition count for a topic
    ///
    /// # Arguments
    ///
    /// * `topic_id` - The unique identifier for the topic
    ///
    /// # Returns
    ///
    /// * `Ok(u32)` - The partition count if the topic exists
    /// * `Err(TopicRegistryError)` - If the topic does not exist
    pub fn get_partition_count(&self, topic_id: &TopicId) -> Result<u32, TopicRegistryError> {
        let topics = self.topics.read().map_err(|_| TopicRegistryError::LockPoisoned)?;
        
        match topics.get(topic_id) {
            Some(topic) => Ok(topic.partition_count()),
            None => Err(TopicRegistryError::TopicNotFound(topic_id.clone())),
        }
    }

    /// Assigns a partition for a given key using consistent hashing
    ///
    /// # Arguments
    ///
    /// * `topic_id` - The unique identifier for the topic
    /// * `key` - The key to hash for partition assignment
    ///
    /// # Returns
    ///
    /// * `Ok(PartitionId)` - The assigned partition ID
    /// * `Err(TopicRegistryError)` - If the topic does not exist
    pub fn assign_partition<K: Hash>(&self, topic_id: &TopicId, key: &K) -> Result<PartitionId, TopicRegistryError> {
        let topics = self.topics.read().map_err(|_| TopicRegistryError::LockPoisoned)?;
        
        match topics.get(topic_id) {
            Some(topic) => {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                key.hash(&mut hasher);
                let hash = hasher.finish();
                
                let partition_index = (hash % topic.partition_count() as u64) as u32;
                Ok(PartitionId::new(partition_index))
            },
            None => Err(TopicRegistryError::TopicNotFound(topic_id.clone())),
        }
    }
}

/// Error types for topic registry operations
#[derive(Debug, Clone, PartialEq)]
pub enum TopicRegistryError {
    /// Topic already exists in the registry
    TopicAlreadyExists(TopicId),
    /// Topic not found in the registry
    TopicNotFound(TopicId),
    /// Internal lock was poisoned
    LockPoisoned,
}

impl std::fmt::Display for TopicRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopicRegistryError::TopicAlreadyExists(topic_id) => {
                write!(f, "Topic '{}' already exists", topic_id.as_str())
            },
            TopicRegistryError::TopicNotFound(topic_id) => {
                write!(f, "Topic '{}' not found", topic_id.as_str())
            },
            TopicRegistryError::LockPoisoned => {
                write!(f, "Internal lock was poisoned")
            },
        }
    }
}

impl std::error::Error for TopicRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_topic() {
        let registry = TopicRegistry::new();
        let topic_id = TopicId::new("test-topic".to_string());
        
        assert!(registry.create_topic(topic_id.clone(), 4).is_ok());
        
        let topic = registry.get_topic(&topic_id).unwrap().unwrap();
        assert_eq!(topic.id(), &topic_id);
        assert_eq!(topic.partition_count(), 4);
        assert_eq!(topic.partitions().len(), 4);
    }

    #[test]
    fn test_create_duplicate_topic() {
        let registry = TopicRegistry::new();
        let topic_id = TopicId::new("test-topic".to_string());
        
        assert!(registry.create_topic(topic_id.clone(), 4).is_ok());
        assert!(matches!(
            registry.create_topic(topic_id.clone(), 4),
            Err(TopicRegistryError::TopicAlreadyExists(_))
        ));
    }

    #[test]
    fn test_delete_topic() {
        let registry = TopicRegistry::new();
        let topic_id = TopicId::new("test-topic".to_string());
        
        registry.create_topic(topic_id.clone(), 4).unwrap();
        assert!(registry.delete_topic(&topic_id).is_ok());
        assert!(registry.get_topic(&topic_id).unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_topic() {
        let registry = TopicRegistry::new();
        let topic_id = TopicId::new("nonexistent-topic".to_string());
        
        assert!(matches!(
            registry.delete_topic(&topic_id),
            Err(TopicRegistryError::TopicNotFound(_))
        ));
    }

    #[test]
    fn test_list_topics() {
        let registry = TopicRegistry::new();
        
        registry.create_topic(TopicId::new("topic1".to_string()), 2).unwrap();
        registry.create_topic(TopicId::new("topic2".to_string()), 3).unwrap();
        
        let topics = registry.list_topics().unwrap();
        assert_eq!(topics.len(), 2);
    }

    #[test]
    fn test_get_partition_count() {
        let registry = TopicRegistry::new();
        let topic_id = TopicId::new("test-topic".to_string());
        
        registry.create_topic(topic_id.clone(), 8).unwrap();
        assert_eq!(registry.get_partition_count(&topic_id).unwrap(), 8);
    }

    #[test]
    fn test_assign_partition() {
        let registry = TopicRegistry::new();
        let topic_id = TopicId::new("test-topic".to_string());
        
        registry.create_topic(topic_id.clone(), 4).unwrap();
        
        let partition1 = registry.assign_partition(&topic_id, &"key1").unwrap();
        let partition2 = registry.assign_partition(&topic_id, &"key2").unwrap();
        
        assert!(partition1.as_u32() < 4);
        assert!(partition2.as_u32() < 4);
    }

    #[test]
    fn test_assign_partition_nonexistent_topic() {
        let registry = TopicRegistry::new();
        let topic_id = TopicId::new("nonexistent-topic".to_string());
        
        assert!(matches!(
            registry.assign_partition(&topic_id, &"key"),
            Err(TopicRegistryError::TopicNotFound(_))
        ));
    }
}
