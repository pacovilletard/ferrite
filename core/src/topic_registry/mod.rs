//! Topic registry implementation for Ferrite broker
//!
//! This module provides the topic management system with support for multiple
//! partitions per topic, as specified in issue #4.

mod topic_registry;
pub use topic_registry::*;
