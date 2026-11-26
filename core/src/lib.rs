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

pub mod foundation {
    use alloc::{collections::{BTreeSet,VecDeque,BTreeMap},vec::Vec,string::String};
    #[derive(Debug,PartialEq,Eq)] pub enum Fault { Invalid, Exhausted, Denied, Conflict, Verification }
    pub const PAGE:u64=4096;
    pub struct Frames { free:BTreeSet<u64>, used:BTreeSet<u64> }
    impl Frames {
        pub fn new(start:u64,end:u64)->Result<Self,Fault>{
            if start>=end || start%PAGE!=0 || end%PAGE!=0{return Err(Fault::Invalid)}
            Ok(Self{free:(start..end).step_by(PAGE as usize).collect(),used:BTreeSet::new()})
        }
        pub fn reserve(&mut self,start:u64,end:u64)->Result<(),Fault>{
            if start>=end{return Err(Fault::Invalid)}
            if self.used.iter().any(|p|*p<end && p.saturating_add(PAGE)>start){return Err(Fault::Conflict)}
            self.free.retain(|p|!(*p<end && p.saturating_add(PAGE)>start));Ok(())
        }
        pub fn allocate(&mut self)->Result<u64,Fault>{
            let p=*self.free.iter().next().ok_or(Fault::Exhausted)?;
            self.free.remove(&p);self.used.insert(p);Ok(p)
        }
        pub fn release(&mut self,p:u64)->Result<(),Fault>{
            if !self.used.remove(&p){return Err(Fault::Invalid)}
            self.free.insert(p);Ok(())
        }
    }
    #[derive(Default)]pub struct Scheduler{ready:VecDeque<u64>,known:BTreeSet<u64>,running:Option<u64>}
    impl Scheduler{
        pub fn add(&mut self,id:u64)->bool{if !self.known.insert(id){return false}self.ready.push_back(id);true}
        pub fn tick(&mut self)->Option<u64>{if let Some(id)=self.running.take(){self.ready.push_back(id)}self.running=self.ready.pop_front();self.running}
        pub fn remove(&mut self,id:u64)->bool{if !self.known.remove(&id){return false}self.ready.retain(|x|*x!=id);if self.running==Some(id){self.running=None}true}
    }
    pub struct Channel{sender:u64,receiver:u64,limit:usize,max:usize,queue:VecDeque<Vec<u8>>}
    impl Channel{
        pub fn new(sender:u64,receiver:u64,limit:usize,max:usize)->Self{Self{sender,receiver,limit,max,queue:VecDeque::new()}}
        pub fn send(&mut self,actor:u64,data:&[u8])->Result<(),Fault>{if actor!=self.sender{return Err(Fault::Denied)}if data.len()>self.max{return Err(Fault::Invalid)}if self.queue.len()>=self.limit{return Err(Fault::Exhausted)}self.queue.push_back(data.to_vec());Ok(())}
        pub fn receive(&mut self,actor:u64)->Result<Option<Vec<u8>>,Fault>{if actor!=self.receiver{return Err(Fault::Denied)}Ok(self.queue.pop_front())}
    }
    #[derive(Default)]pub struct Store{revision:u64,values:BTreeMap<String,Vec<u8>>}
    impl Store{
        pub fn get(&self,k:&str)->Option<&[u8]>{self.values.get(k).map(Vec::as_slice)}
        pub fn commit(&mut self,expected:u64,changes:&[(String,Vec<u8>)],allow:impl Fn(&str)->bool,verify:impl Fn(&BTreeMap<String,Vec<u8>>)->bool)->Result<u64,Fault>{
            if self.revision!=expected{return Err(Fault::Conflict)}
            let next=expected.checked_add(1).ok_or(Fault::Conflict)?;
            let mut staged=self.values.clone();
            for (k,v) in changes{if !allow(k){return Err(Fault::Denied)}staged.insert(k.clone(),v.clone());}
            if !verify(&staged){return Err(Fault::Verification)}
            self.values=staged;self.revision=next;Ok(next)
        }
    }
    #[cfg(test)]mod tests{
        use super::*;
        #[test]fn integrated_reference(){
            let mut f=Frames::new(4096,16384).unwrap();f.reserve(8192,12288).unwrap();
            let a=f.allocate().unwrap();let b=f.allocate().unwrap();assert_eq!((a,b),(4096,12288));assert_eq!(f.allocate(),Err(Fault::Exhausted));
            let mut s=Scheduler::default();assert!(s.add(1));assert!(s.add(2));assert_eq!(s.tick(),Some(1));assert_eq!(s.tick(),Some(2));
            let mut c=Channel::new(1,2,1,8);assert_eq!(c.send(2,b"x"),Err(Fault::Denied));c.send(1,b"ok").unwrap();assert_eq!(c.receive(2).unwrap(),Some(b"ok".to_vec()));
            let mut t=Store::default();let changes=[("key".into(),alloc::vec![1])];assert_eq!(t.commit(0,&changes,|_|true,|_|false),Err(Fault::Verification));assert_eq!(t.get("key"),None);assert_eq!(t.commit(0,&changes,|_|true,|_|true),Ok(1));
            assert!(s.remove(1));f.release(a).unwrap();assert_eq!(f.release(a),Err(Fault::Invalid));
        }
    }
}

pub mod graphics {
    #[derive(Debug,PartialEq,Eq)] pub enum Error { Geometry, Bounds }
    pub struct Surface<'a>{ pixels:&'a mut [u32], width:usize,height:usize,stride:usize }
    impl<'a> Surface<'a>{
        pub fn new(pixels:&'a mut [u32],width:usize,height:usize,stride:usize)->Result<Self,Error>{
            if width==0 || height==0 || stride<width || stride.checked_mul(height).map_or(true,|n|n>pixels.len()){return Err(Error::Geometry)}
            Ok(Self{pixels,width,height,stride})
        }
        pub fn fill(&mut self,x:usize,y:usize,w:usize,h:usize,color:u32)->Result<(),Error>{
            let right=x.checked_add(w).ok_or(Error::Bounds)?;
            let bottom=y.checked_add(h).ok_or(Error::Bounds)?;
            if right>self.width || bottom>self.height{return Err(Error::Bounds)}
            for row in y..bottom{self.pixels[row*self.stride+x..row*self.stride+right].fill(color)}
            Ok(())
        }
    }
    #[cfg(test)]mod tests{use super::*;#[test]fn clipped(){let mut p=[0;12];{let mut s=Surface::new(&mut p,3,3,4).unwrap();s.fill(1,1,2,2,7).unwrap();assert_eq!(s.fill(2,2,2,1,9),Err(Error::Bounds));}assert_eq!(p,[0,0,0,0,0,7,7,0,0,7,7,0]);}}
}
pub mod block {
    #[derive(Debug,PartialEq,Eq)]pub enum Error{Geometry,Bounds,Size}
    pub trait Device{fn block_size(&self)->usize;fn blocks(&self)->usize;fn read(&self,index:usize,out:&mut[u8])->Result<(),Error>;fn write(&mut self,index:usize,input:&[u8])->Result<(),Error>;}
    pub struct RamDisk<'a>{data:&'a mut[u8],size:usize}
    impl<'a> RamDisk<'a>{pub fn new(data:&'a mut[u8],size:usize)->Result<Self,Error>{if size==0||data.len()%size!=0{return Err(Error::Geometry)}Ok(Self{data,size})}}
    impl Device for RamDisk<'_>{
        fn block_size(&self)->usize{self.size}
        fn blocks(&self)->usize{self.data.len()/self.size}
        fn read(&self,index:usize,out:&mut[u8])->Result<(),Error>{if out.len()!=self.size{return Err(Error::Size)}if index>=self.blocks(){return Err(Error::Bounds)}let start=index*self.size;out.copy_from_slice(&self.data[start..start+self.size]);Ok(())}
        fn write(&mut self,index:usize,input:&[u8])->Result<(),Error>{if input.len()!=self.size{return Err(Error::Size)}if index>=self.blocks(){return Err(Error::Bounds)}let start=index*self.size;self.data[start..start+self.size].copy_from_slice(input);Ok(())}
    }
    #[cfg(test)]mod tests{use super::*;#[test]fn block_io(){let mut data=[0;16];let mut d=RamDisk::new(&mut data,4).unwrap();d.write(2,b"test").unwrap();let mut out=[0;4];d.read(2,&mut out).unwrap();assert_eq!(&out,b"test");assert_eq!(d.write(4,b"test"),Err(Error::Bounds));}}
}
pub mod net {
    #[derive(Debug,PartialEq,Eq)]pub enum Error{Truncated,Version,Header,Length,Checksum}
    pub fn checksum(bytes:&[u8])->u16{let mut sum=0u32;for pair in bytes.chunks(2){sum+=((pair[0] as u32)<<8)|pair.get(1).copied().unwrap_or(0) as u32;sum=(sum&65535)+(sum>>16);}!(sum as u16)}
    pub struct Ipv4<'a>{pub source:[u8;4],pub destination:[u8;4],pub protocol:u8,pub payload:&'a[u8]}
    pub fn parse_ipv4(p:&[u8])->Result<Ipv4<'_>,Error>{
        if p.len()<20{return Err(Error::Truncated)}if p[0]>>4!=4{return Err(Error::Version)}
        let h=(p[0]&15) as usize*4;if h<20||h>p.len(){return Err(Error::Header)}
        let n=u16::from_be_bytes([p[2],p[3]]) as usize;if n<h||n>p.len(){return Err(Error::Length)}
        if checksum(&p[..h])!=0{return Err(Error::Checksum)}
        Ok(Ipv4{source:[p[12],p[13],p[14],p[15]],destination:[p[16],p[17],p[18],p[19]],protocol:p[9],payload:&p[h..n]})
    }
    #[cfg(test)]mod tests{use super::*;#[test]fn packet(){let mut p=[0u8;21];p[0]=0x45;p[3]=21;p[8]=64;p[9]=17;p[20]=7;let c=checksum(&p[..20]).to_be_bytes();p[10]=c[0];p[11]=c[1];assert_eq!(parse_ipv4(&p).unwrap().payload,&[7]);p[8]=1;assert!(matches!(parse_ipv4(&p),Err(Error::Checksum)));}}
}

pub mod intent {
    //! Strict typed command parser. AI proposals never execute as shell commands.
    use super::{Action,Pool,Operation};
    use alloc::vec::Vec;
    #[derive(Debug,PartialEq,Eq)]pub enum Error{Invalid,TooMany}
    pub fn parse(input:&str)->Result<Vec<Action>,Error>{
        if input.len()>8192{return Err(Error::TooMany)}
        let mut out=Vec::new();
        for line in input.lines().filter(|l|!l.trim().is_empty()){
            if out.len()>=64{return Err(Error::TooMany)}
            let mut p=line.split_ascii_whitespace();
            let number=|s:Option<&str>|s.and_then(|v|v.parse::<u64>().ok()).ok_or(Error::Invalid);
            let a=match p.next(){
                Some("spawn")=>{let id=number(p.next())?;let pool=match p.next(){Some("ram")=>Pool::Ram,Some("vram")=>Pool::Vram,Some("unified")=>Pool::Unified,_=>return Err(Error::Invalid)};let bytes=number(p.next())?;if bytes==0{return Err(Error::Invalid)}Action::Spawn{id,pool,bytes}},
                Some("stop")=>Action::Stop{id:number(p.next())?},
                Some("app")=>{let app=number(p.next())?;let operation=match p.next(){Some("observe")=>Operation::Observe,Some("navigate")=>Operation::Navigate,Some("modify")=>Operation::Modify,_=>return Err(Error::Invalid)};Action::App{app,operation}},
                _=>return Err(Error::Invalid)
            };
            if p.next().is_some(){return Err(Error::Invalid)}out.push(a);
        }
        if out.is_empty(){return Err(Error::Invalid)}Ok(out)
    }
    #[cfg(test)]mod tests{use super::*;#[test]fn strict(){assert_eq!(parse("spawn 1 ram 64\nstop 1").unwrap().len(),2);assert_eq!(parse("spawn 1 ram 0"),Err(Error::Invalid));assert_eq!(parse("app 1 modify extra"),Err(Error::Invalid));assert_eq!(parse("shell rm"),Err(Error::Invalid));}}
}
pub mod workflow {
    //! Reference broker: each action must pass existing authorization; no automatic grant.
    use super::{Action,System,Error};
    use alloc::vec::Vec;
    #[derive(Debug,PartialEq,Eq)]pub struct Step{pub token:u64,pub action:Action}
    #[derive(Debug,PartialEq,Eq)]pub struct Report{pub results:Vec<Result<(),Error>>,pub unexecuted:usize}
    pub fn run(system:&mut System,actor:u64,steps:&[Step])->Report{
        let mut results=Vec::new();
        for s in steps{let r=system.execute(actor,s.token,s.action.clone());results.push(r);if r.is_err(){break}}
        Report{unexecuted:steps.len()-results.len(),results}
    }
    #[cfg(test)]mod tests{use super::*;use crate::{Mode,Pool,Scope};#[test]fn no_implicit_permission(){let mut s=System::new(Mode::Local,&[(Pool::Ram,100)]);s.grant(1,7,Scope::Process,1).unwrap();let r=run(&mut s,7,&[Step{token:1,action:Action::Spawn{id:1,pool:Pool::Ram,bytes:10}},Step{token:1,action:Action::Stop{id:1}},Step{token:1,action:Action::Stop{id:1}}]);assert_eq!(r.results,alloc::vec![Ok(()),Err(Error::Denied)]);assert_eq!(r.unexecuted,1);assert_eq!(s.used(Pool::Ram),10);}}
}
