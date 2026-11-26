#![no_std]
extern crate alloc;
use alloc::{collections::BTreeMap, vec::Vec, string::String};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pool { Ram, Vram, Unified }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Local, Cloud, Hybrid, Manual }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Denied, Exhausted, Missing, Duplicate, Invalid, Manual }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Spawn { id: u64, pool: Pool, bytes: u64 },
    Stop { id: u64 },
    App { app: u64, operation: Operation },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation { Observe, Navigate, Modify }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope { Process, App { app: u64, operation: Operation } }
#[derive(Clone, Debug)]
struct Grant { actor: u64, scope: Scope, remaining: u64 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Process { pub pool: Pool, pub bytes: u64, pub owner: u64 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event { pub actor: u64, pub action: Action, pub result: Result<(), Error> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppRequest { pub actor: u64, pub app: u64, pub operation: Operation }
pub struct System {
    mode: Mode,
    capacity: BTreeMap<Pool, u64>,
    used: BTreeMap<Pool, u64>,
    processes: BTreeMap<u64, Process>,
    grants: BTreeMap<u64, Grant>,
    events: Vec<Event>,
    requests: Vec<AppRequest>,
}
impl System {
    pub fn new(mode: Mode, pools: &[(Pool,u64)]) -> Self {
        Self { mode, capacity: pools.iter().copied().collect(), used: BTreeMap::new(),
            processes: BTreeMap::new(), grants: BTreeMap::new(), events: Vec::new(), requests: Vec::new() }
    }
    // Trusted user-session service only; do not expose directly to agent transport.
    pub fn grant(&mut self, token: u64, actor: u64, scope: Scope, uses: u64) -> Result<(),Error> {
        if uses == 0 { return Err(Error::Invalid); }
        if self.grants.contains_key(&token) { return Err(Error::Duplicate); }
        self.grants.insert(token, Grant {actor,scope,remaining:uses}); Ok(())
    }
    pub fn revoke(&mut self, token: u64) { self.grants.remove(&token); }
    pub fn set_mode(&mut self, mode: Mode) { self.mode=mode; }
    pub fn events(&self) -> &[Event] { &self.events }
    pub fn requests(&self) -> &[AppRequest] { &self.requests }
    pub fn used(&self, pool: Pool) -> u64 { *self.used.get(&pool).unwrap_or(&0) }
    pub fn process(&self, id: u64) -> Option<&Process> { self.processes.get(&id) }
    fn authorize(&self, actor:u64, token:u64, action:&Action) -> Result<(),Error> {
        if self.mode == Mode::Manual { return Err(Error::Manual); }
        let g=self.grants.get(&token).ok_or(Error::Denied)?;
        if g.actor!=actor || g.remaining==0 { return Err(Error::Denied); }
        match (&g.scope,action) {
            (Scope::Process,Action::Spawn{..}|Action::Stop{..}) => Ok(()),
            (Scope::App{app,operation},Action::App{app:a,operation:o}) if app==a && operation==o => Ok(()),
            _=>Err(Error::Denied)
        }
    }
    fn apply(&mut self, actor:u64, action:&Action)->Result<(),Error>{
        match *action {
            Action::Spawn{id,pool,bytes}=>{
                if bytes==0 {return Err(Error::Invalid)}
                if self.processes.contains_key(&id){return Err(Error::Duplicate)}
                let next=self.used(pool).checked_add(bytes).ok_or(Error::Exhausted)?;
                if next>*self.capacity.get(&pool).unwrap_or(&0){return Err(Error::Exhausted)}
                self.used.insert(pool,next);
                self.processes.insert(id,Process{pool,bytes,owner:actor}); Ok(())
            },
            Action::Stop{id}=>{
                let p=self.processes.get(&id).ok_or(Error::Missing)?;
                if p.owner!=actor{return Err(Error::Denied)}
                let p=self.processes.remove(&id).ok_or(Error::Missing)?;
                self.used.insert(p.pool,self.used(p.pool)-p.bytes); Ok(())
            },
            Action::App{app,operation}=>{
                // Queues an authorized request; no claim of third-party GUI execution.
                self.requests.push(AppRequest{actor,app,operation}); Ok(())
            }
        }
    }
    pub fn execute(&mut self,actor:u64,token:u64,action:Action)->Result<(),Error>{
        let result=self.authorize(actor,token,&action).and_then(|_|self.apply(actor,&action));
        if result.is_ok(){if let Some(g)=self.grants.get_mut(&token){g.remaining-=1;}}
        self.events.push(Event{actor,action,result}); result
    }
}
pub trait Planner { fn propose(&self, intent:&str)->Result<Vec<Action>,String>; }
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn integrated_lifecycle(){
        let mut s=System::new(Mode::Hybrid,&[(Pool::Ram,100),(Pool::Vram,20)]);
        s.grant(1,7,Scope::Process,2).unwrap();
        assert_eq!(s.execute(7,1,Action::Spawn{id:1,pool:Pool::Ram,bytes:80}),Ok(()));
        assert_eq!(s.execute(7,1,Action::Spawn{id:2,pool:Pool::Ram,bytes:30}),Err(Error::Exhausted));
        assert_eq!(s.used(Pool::Ram),80);
        assert_eq!(s.execute(7,1,Action::Stop{id:1}),Ok(()));
        assert_eq!(s.used(Pool::Ram),0);
        assert_eq!(s.execute(7,1,Action::Spawn{id:2,pool:Pool::Ram,bytes:1}),Err(Error::Denied));
        assert_eq!(s.events().len(),4);
    }
    #[test] fn app_scoped_revocation(){
        let mut s=System::new(Mode::Local,&[]);
        s.grant(5,7,Scope::App{app:9,operation:Operation::Observe},2).unwrap();
        assert_eq!(s.execute(7,5,Action::App{app:9,operation:Operation::Modify}),Err(Error::Denied));
        assert_eq!(s.execute(8,5,Action::App{app:9,operation:Operation::Observe}),Err(Error::Denied));
        assert_eq!(s.execute(7,5,Action::App{app:9,operation:Operation::Observe}),Ok(()));
        s.revoke(5);
        assert_eq!(s.execute(7,5,Action::App{app:9,operation:Operation::Observe}),Err(Error::Denied));
        assert_eq!(s.requests().len(),1);
    }
    #[test] fn manual_and_separate_pools(){
        let mut s=System::new(Mode::Manual,&[(Pool::Unified,100),(Pool::Vram,10)]);
        s.grant(1,1,Scope::Process,3).unwrap();
        let a=Action::Spawn{id:1,pool:Pool::Vram,bytes:11};
        assert_eq!(s.execute(1,1,a.clone()),Err(Error::Manual));
        s.set_mode(Mode::Local);
        assert_eq!(s.execute(1,1,a),Err(Error::Exhausted));
        assert_eq!(s.used(Pool::Unified),0);
    }
}
