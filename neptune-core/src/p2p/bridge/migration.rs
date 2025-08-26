//! Migration management for transitioning from legacy to libp2p networking
//! 
//! This module manages the gradual migration of peers from the legacy
//! TCP-based networking to the new libp2p implementation.

use super::{BridgeError, BridgeStatus, MigrationPhase};
use crate::p2p::config::NetworkConfig;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;

/// Migration manager for coordinating the transition
pub struct MigrationManager {
    /// Network configuration
    config: NetworkConfig,
    /// Bridge status reference
    status: Arc<RwLock<BridgeStatus>>,
    /// Migration timer
    migration_timer: Option<tokio::task::JoinHandle<()>>,
    /// Migration start time
    start_time: Option<Instant>,
    /// Target migration duration
    target_duration: Duration,
}

impl MigrationManager {
    /// Create a new migration manager
    pub fn new(config: NetworkConfig, status: Arc<RwLock<BridgeStatus>>) -> Self {
        Self {
            config,
            status,
            migration_timer: None,
            start_time: None,
            target_duration: Duration::from_secs(3600), // 1 hour default
        }
    }

    /// Start the migration manager
    pub async fn start(&mut self) -> Result<(), BridgeError> {
        if self.migration_timer.is_some() {
            return Err(BridgeError::MigrationError(
                "Migration manager already started".to_string()
            ));
        }

        // Start migration timer
        let status = self.status.clone();
        let target_duration = self.target_duration;
        
        let timer_handle = tokio::spawn(async move {
            Self::migration_timer_loop(status, target_duration).await;
        });

        self.migration_timer = Some(timer_handle);
        self.start_time = Some(Instant::now());

        Ok(())
    }

    /// Stop the migration manager
    pub async fn stop(&mut self) -> Result<(), BridgeError> {
        if let Some(timer) = self.migration_timer.take() {
            timer.abort();
        }

        self.start_time = None;
        Ok(())
    }

    /// Advance to the next migration phase
    pub async fn advance_phase(&mut self) -> Result<(), BridgeError> {
        let mut status = self.status.write().await;
        
        let new_phase = match status.migration_phase {
            MigrationPhase::DualProtocol => MigrationPhase::Migration,
            MigrationPhase::Migration => MigrationPhase::Libp2pPrimary,
            MigrationPhase::Libp2pPrimary => MigrationPhase::Complete,
            MigrationPhase::Complete => {
                return Err(BridgeError::MigrationError(
                    "Already in final migration phase".to_string()
                ));
            }
        };

        status.migration_phase = new_phase;
        status.migration_progress = self.calculate_progress(&new_phase);

        tracing::info!("Migration phase advanced to: {:?}", new_phase);
        Ok(())
    }

    /// Get current migration phase
    pub async fn current_phase(&self) -> MigrationPhase {
        let status = self.status.read().await;
        status.migration_phase.clone()
    }

    /// Get migration progress percentage
    pub async fn progress(&self) -> f64 {
        let status = self.status.read().await;
        status.migration_progress
    }

    /// Check if migration is complete
    pub async fn is_complete(&self) -> bool {
        let status = self.status.read().await;
        matches!(status.migration_phase, MigrationPhase::Complete)
    }

    /// Set target migration duration
    pub fn set_target_duration(&mut self, duration: Duration) {
        self.target_duration = duration;
    }

    /// Get estimated time remaining
    pub async fn time_remaining(&self) -> Option<Duration> {
        let start_time = self.start_time?;
        let elapsed = start_time.elapsed();
        
        if elapsed < self.target_duration {
            Some(self.target_duration - elapsed)
        } else {
            None
        }
    }

    /// Calculate progress percentage for a given phase
    fn calculate_progress(&self, phase: &MigrationPhase) -> f64 {
        match phase {
            MigrationPhase::DualProtocol => 0.0,
            MigrationPhase::Migration => 25.0,
            MigrationPhase::Libp2pPrimary => 75.0,
            MigrationPhase::Complete => 100.0,
        }
    }

    /// Migration timer loop for automatic phase progression
    async fn migration_timer_loop(status: Arc<RwLock<BridgeStatus>>, target_duration: Duration) {
        let mut interval = interval(Duration::from_secs(60)); // Check every minute
        
        loop {
            interval.tick().await;
            
            let mut status_guard = status.write().await;
            let current_phase = &status_guard.migration_phase;
            
            // Auto-advance phases based on time and conditions
            match current_phase {
                MigrationPhase::DualProtocol => {
                    // Stay in dual protocol mode for initial period
                    // This allows both networks to stabilize
                    continue;
                }
                MigrationPhase::Migration => {
                    // Gradually increase progress during migration
                    status_guard.migration_progress = 
                        (status_guard.migration_progress + 1.0).min(50.0);
                }
                MigrationPhase::Libp2pPrimary => {
                    // Continue progress toward completion
                    status_guard.migration_progress = 
                        (status_guard.migration_progress + 1.0).min(90.0);
                }
                MigrationPhase::Complete => {
                    // Migration complete, no further action needed
                    break;
                }
            }
        }
    }

    /// Get migration statistics
    pub async fn get_statistics(&self) -> MigrationStatistics {
        let status = self.status.read().await;
        let start_time = self.start_time;
        
        MigrationStatistics {
            current_phase: status.migration_phase.clone(),
            progress: status.migration_progress,
            legacy_connections: status.legacy_connections,
            libp2p_connections: status.libp2p_connections,
            start_time,
            target_duration: self.target_duration,
        }
    }
}

/// Migration statistics for monitoring
#[derive(Debug, Clone)]
pub struct MigrationStatistics {
    /// Current migration phase
    pub current_phase: MigrationPhase,
    /// Migration progress percentage
    pub progress: f64,
    /// Number of legacy connections
    pub legacy_connections: usize,
    /// Number of libp2p connections
    pub libp2p_connections: usize,
    /// Migration start time
    pub start_time: Option<Instant>,
    /// Target migration duration
    pub target_duration: Duration,
}

impl MigrationStatistics {
    /// Get total connection count
    pub fn total_connections(&self) -> usize {
        self.legacy_connections + self.libp2p_connections
    }

    /// Get connection distribution percentage
    pub fn connection_distribution(&self) -> (f64, f64) {
        let total = self.total_connections() as f64;
        if total == 0.0 {
            (0.0, 0.0)
        } else {
            let legacy_pct = (self.legacy_connections as f64 / total) * 100.0;
            let libp2p_pct = (self.libp2p_connections as f64 / total) * 100.0;
            (legacy_pct, libp2p_pct)
        }
    }

    /// Check if migration is on track
    pub fn is_on_track(&self) -> bool {
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed();
            let expected_progress = (elapsed.as_secs_f64() / self.target_duration.as_secs_f64()) * 100.0;
            self.progress >= expected_progress * 0.8 // Allow 20% variance
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::config::NetworkConfig;

    #[tokio::test]
    async fn test_migration_manager_creation() {
        let config = NetworkConfig::test_config();
        let status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));
        
        let manager = MigrationManager::new(config, status);
        assert_eq!(manager.current_phase().await, MigrationPhase::DualProtocol);
    }

    #[tokio::test]
    async fn test_migration_phase_advancement() {
        let config = NetworkConfig::test_config();
        let status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));
        
        let mut manager = MigrationManager::new(config, status);
        
        // Start in dual protocol mode
        assert_eq!(manager.current_phase().await, MigrationPhase::DualProtocol);
        
        // Advance to migration phase
        assert!(manager.advance_phase().await.is_ok());
        assert_eq!(manager.current_phase().await, MigrationPhase::Migration);
        
        // Advance to libp2p primary
        assert!(manager.advance_phase().await.is_ok());
        assert_eq!(manager.current_phase().await, MigrationPhase::Libp2pPrimary);
        
        // Advance to complete
        assert!(manager.advance_phase().await.is_ok());
        assert_eq!(manager.current_phase().await, MigrationPhase::Complete);
        
        // Cannot advance further
        assert!(manager.advance_phase().await.is_err());
    }

    #[tokio::test]
    async fn test_migration_statistics() {
        let config = NetworkConfig::test_config();
        let status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::Migration,
            legacy_connections: 5,
            libp2p_connections: 15,
            migration_progress: 25.0,
        }));
        
        let manager = MigrationManager::new(config, status);
        let stats = manager.get_statistics().await;
        
        assert_eq!(stats.total_connections(), 20);
        assert_eq!(stats.connection_distribution(), (25.0, 75.0));
        assert_eq!(stats.progress, 25.0);
    }

    #[test]
    fn test_migration_phase_progress_calculation() {
        let config = NetworkConfig::test_config();
        let status = Arc::new(RwLock::new(BridgeStatus {
            migration_phase: MigrationPhase::DualProtocol,
            legacy_connections: 0,
            libp2p_connections: 0,
            migration_progress: 0.0,
        }));
        
        let manager = MigrationManager::new(config, status);
        
        // Test progress calculation for each phase
        assert_eq!(manager.calculate_progress(&MigrationPhase::DualProtocol), 0.0);
        assert_eq!(manager.calculate_progress(&MigrationPhase::Migration), 25.0);
        assert_eq!(manager.calculate_progress(&MigrationPhase::Libp2pPrimary), 75.0);
        assert_eq!(manager.calculate_progress(&MigrationPhase::Complete), 100.0);
    }
}
