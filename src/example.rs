use libtxc::LibTxc;
use std::env;

fn main() {
    let connect = "<command id=\"connect\">\
                    <login></login>\
                    <password></password>\
			        <milliseconds>true</milliseconds>\
			        <autopos>false</autopos>\
			        <rqdelay>10</rqdelay>\
                    <host>tr1.finam.ru</host>\
                    <port>3900</port>
                  </command>";
    let disconnect = "<command id=\"disconnect\"/>";
    std::thread::spawn(move || {
        println!("loading");
        let mut lib: LibTxc = Default::default();

        println!("initializing");
        let cd = env::current_dir().unwrap();
        lib.initialize(cd, Default::default()).unwrap();

        println!("set_callback");
        lib.set_callback(|buff| println!("{}", Into::<String>::into(buff)));

        println!("connecting");
        lib.send_command(connect).unwrap();

        std::thread::sleep(std::time::Duration::new(30, 0));

        println!("disconnecting");
        lib.send_command(disconnect).unwrap();

        lib.uninitialize().unwrap();
    })
    .join()
    .unwrap();
}
