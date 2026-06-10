use aurelia_common::ChunkPos;
use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::thread::{self, ThreadId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId {
    pub x: i32,
    pub z: i32,
}

impl RegionId {
    pub const CHUNKS_PER_REGION: i32 = 8;

    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    pub fn from_chunk(pos: ChunkPos) -> Self {
        Self::new(
            pos.x.div_euclid(Self::CHUNKS_PER_REGION),
            pos.z.div_euclid(Self::CHUNKS_PER_REGION),
        )
    }
}

impl fmt::Display for RegionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RegionId[x={}, z={}]", self.x, self.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionState {
    Created,
    Ticking,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionOwnershipError {
    message: String,
}

impl RegionOwnershipError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RegionOwnershipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for RegionOwnershipError {}

#[derive(Debug)]
pub struct RegionThreadContext {
    region_id: RegionId,
    owner_thread: Option<ThreadId>,
}

impl RegionThreadContext {
    pub const fn new(region_id: RegionId) -> Self {
        Self {
            region_id,
            owner_thread: None,
        }
    }

    pub const fn region_id(&self) -> RegionId {
        self.region_id
    }

    pub fn bind_to_current_thread(&mut self) {
        self.owner_thread = Some(thread::current().id());
    }

    pub fn clear_owner(&mut self) {
        self.owner_thread = None;
    }

    pub fn is_owned_by_current_thread(&self) -> bool {
        self.owner_thread == Some(thread::current().id())
    }

    pub fn assert_owned_by_current_thread(&self) -> Result<(), RegionOwnershipError> {
        match self.owner_thread {
            None => Err(RegionOwnershipError::new(format!(
                "Region {} has no owning thread",
                self.region_id
            ))),
            Some(owner) if owner != thread::current().id() => Err(RegionOwnershipError::new(
                format!("Region {} is owned by another thread", self.region_id),
            )),
            Some(_) => Ok(()),
        }
    }
}

pub type RegionTask = Box<dyn FnOnce(&mut Region) + Send + 'static>;

#[derive(Default)]
pub struct RegionMailbox {
    tasks: VecDeque<RegionTask>,
}

impl fmt::Debug for RegionMailbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegionMailbox")
            .field("pending_tasks", &self.pending_tasks())
            .finish()
    }
}

impl RegionMailbox {
    pub fn submit<F>(&mut self, task: F)
    where
        F: FnOnce(&mut Region) + Send + 'static,
    {
        self.tasks.push_back(Box::new(task));
    }

    pub fn pending_tasks(&self) -> usize {
        self.tasks.len()
    }

    fn pop(&mut self) -> Option<RegionTask> {
        self.tasks.pop_front()
    }
}

#[derive(Debug)]
pub struct Region {
    id: RegionId,
    thread_context: RegionThreadContext,
    mailbox: RegionMailbox,
    state: RegionState,
}

impl Region {
    pub fn new(id: RegionId) -> Self {
        Self {
            id,
            thread_context: RegionThreadContext::new(id),
            mailbox: RegionMailbox::default(),
            state: RegionState::Created,
        }
    }

    pub const fn id(&self) -> RegionId {
        self.id
    }

    pub const fn state(&self) -> RegionState {
        self.state
    }

    pub const fn thread_context(&self) -> &RegionThreadContext {
        &self.thread_context
    }

    pub const fn mailbox(&self) -> &RegionMailbox {
        &self.mailbox
    }

    pub fn mailbox_mut(&mut self) -> &mut RegionMailbox {
        &mut self.mailbox
    }

    pub fn begin_tick_on_current_thread(&mut self) {
        self.thread_context.bind_to_current_thread();
        self.state = RegionState::Ticking;
    }

    pub fn end_tick(&mut self) {
        self.state = RegionState::Created;
        self.thread_context.clear_owner();
    }

    pub fn stop(&mut self) {
        self.state = RegionState::Stopped;
        self.thread_context.clear_owner();
    }

    pub fn run_owned(&mut self, task: RegionTask) -> Result<(), RegionOwnershipError> {
        self.thread_context.assert_owned_by_current_thread()?;
        task(self);
        Ok(())
    }

    pub fn drain_mailbox(&mut self) -> Result<usize, RegionOwnershipError> {
        let mut executed = 0;
        while let Some(task) = self.mailbox.pop() {
            self.run_owned(task)?;
            executed += 1;
        }
        Ok(executed)
    }
}

#[derive(Debug, Default)]
pub struct RegionScheduler {
    regions: HashMap<RegionId, Region>,
}

impl RegionScheduler {
    pub fn create_region(&mut self, id: RegionId) -> &mut Region {
        self.regions.entry(id).or_insert_with(|| Region::new(id))
    }

    pub fn region(&self, id: RegionId) -> Option<&Region> {
        self.regions.get(&id)
    }

    pub fn submit<F>(&mut self, id: RegionId, task: F)
    where
        F: FnOnce(&mut Region) + Send + 'static,
    {
        self.create_region(id).mailbox_mut().submit(task);
    }

    pub fn tick_region(&mut self, id: RegionId) -> Result<usize, RegionOwnershipError> {
        let region = self.create_region(id);
        region.begin_tick_on_current_thread();
        let result = region.drain_mailbox();
        region.end_tick();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn current_owner_thread_may_access_region() {
        let mut context = RegionThreadContext::new(RegionId::new(0, 0));
        context.bind_to_current_thread();

        assert!(context.assert_owned_by_current_thread().is_ok());
    }

    #[test]
    fn wrong_thread_access_throws() {
        let mut context = RegionThreadContext::new(RegionId::new(0, 0));
        context.bind_to_current_thread();
        let context = Arc::new(context);
        let other_context = Arc::clone(&context);

        let result =
            std::thread::spawn(move || other_context.assert_owned_by_current_thread()).join();

        assert!(result.unwrap().is_err());
    }

    #[test]
    fn mailbox_runs_tasks_on_owning_thread() {
        let mut region = Region::new(RegionId::new(1, 1));
        let count = Arc::new(Mutex::new(0));
        let first = Arc::clone(&count);
        let second = Arc::clone(&count);

        region
            .mailbox_mut()
            .submit(move |current_region| *first.lock().unwrap() += current_region.id().x);
        region
            .mailbox_mut()
            .submit(move |current_region| *second.lock().unwrap() += current_region.id().z);

        region.begin_tick_on_current_thread();
        let executed = region.drain_mailbox().unwrap();
        region.end_tick();

        assert_eq!(2, executed);
        assert_eq!(2, *count.lock().unwrap());
        assert_eq!(0, region.mailbox().pending_tasks());
    }
}
