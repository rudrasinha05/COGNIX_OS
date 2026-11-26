use cognix_core::{System,Mode,Pool,Scope,Action,Operation};
fn main(){
    let mut system=System::new(Mode::Local,&[(Pool::Ram,1024),(Pool::Vram,256)]);
    system.grant(1,1,Scope::Process,2).expect("grant");
    system.execute(1,1,Action::Spawn{id:1,pool:Pool::Ram,bytes:128}).expect("spawn");
    system.grant(2,1,Scope::App{app:5,operation:Operation::Observe},1).expect("grant");
    system.execute(1,2,Action::App{app:5,operation:Operation::Observe}).expect("queue");
    system.execute(1,1,Action::Stop{id:1}).expect("stop");
    for e in system.events(){println!("{e:?}");}
    assert_eq!(system.used(Pool::Ram),0);
}
