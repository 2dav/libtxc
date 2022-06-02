use libtxc::LibTxc;

fn main() -> Result<(), std::io::Error> {
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

    let mut lib: LibTxc = Default::default();

    let cd = std::env::current_dir()?;

    lib.initialize(cd, Default::default())?;
    lib.set_callback(|buff| println!("{}", buff));
    lib.send_command(connect)?;

    std::thread::sleep(std::time::Duration::new(30, 0));

    lib.send_command(disconnect)?;
    lib.uninitialize()?;

    Ok(())
}
