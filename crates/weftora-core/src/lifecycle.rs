//! Ordered service startup and reverse teardown, independent of adapters.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Boot,
    Load,
    Ready,
    Play,
    Shutdown,
}

pub trait Service {
    fn name(&self) -> &str;
    /// A failed init must leave enough information for `shutdown` to release partially acquired resources.
    fn init(&mut self) -> Result<(), String>;
    fn shutdown(&mut self) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    #[error("cannot {action} while lifecycle is {phase:?}")]
    InvalidPhase { action: &'static str, phase: Phase },
    #[error("service `{service}` failed init: {message}; cleanup errors: {cleanup:?}")]
    Init {
        service: String,
        message: String,
        cleanup: Vec<String>,
    },
    #[error("service shutdown errors: {0:?}")]
    Shutdown(Vec<String>),
}

pub struct Lifecycle {
    services: Vec<Box<dyn Service>>,
    initialized: usize,
    phase: Phase,
    trace: Vec<String>,
}

impl Lifecycle {
    pub fn new(services: Vec<Box<dyn Service>>) -> Self {
        Self {
            services,
            initialized: 0,
            phase: Phase::Boot,
            trace: vec!["boot".into()],
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn trace(&self) -> &[String] {
        &self.trace
    }

    /// Init in declaration order. Failure cleans up the failing service first, then its predecessors.
    pub fn load(&mut self) -> Result<(), LifecycleError> {
        self.require(Phase::Boot, "load")?;
        self.phase = Phase::Load;
        self.trace.push("load".into());
        for index in 0..self.services.len() {
            self.initialized = index + 1;
            let service = self.services[index].name().to_string();
            self.trace.push(format!("init:{service}"));
            if let Err(message) = self.services[index].init() {
                let cleanup = self.teardown();
                return Err(LifecycleError::Init {
                    service,
                    message,
                    cleanup,
                });
            }
        }
        self.phase = Phase::Ready;
        self.trace.push("ready".into());
        Ok(())
    }

    pub fn play(&mut self) -> Result<(), LifecycleError> {
        self.require(Phase::Ready, "play")?;
        self.phase = Phase::Play;
        self.trace.push("play".into());
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), LifecycleError> {
        let errors = self.teardown();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(LifecycleError::Shutdown(errors))
        }
    }

    fn require(&self, phase: Phase, action: &'static str) -> Result<(), LifecycleError> {
        if self.phase == phase {
            Ok(())
        } else {
            Err(LifecycleError::InvalidPhase {
                action,
                phase: self.phase,
            })
        }
    }

    fn teardown(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        while self.initialized > 0 {
            self.initialized -= 1;
            let service = &mut self.services[self.initialized];
            self.trace.push(format!("shutdown:{}", service.name()));
            if let Err(message) = service.shutdown() {
                errors.push(format!("{}: {message}", service.name()));
            }
        }
        if self.phase != Phase::Shutdown {
            self.trace.push("shutdown".into());
        }
        self.phase = Phase::Shutdown;
        errors
    }
}

impl Drop for Lifecycle {
    fn drop(&mut self) {
        self.teardown();
    }
}
