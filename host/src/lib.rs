#[cfg(test)]
mod wasm_test {
    use wasmtime::component::Component;
    use wasmtime::component::Linker;
    use wasmtime::component::ResourceAny;
    use wasmtime::component::Val;
    use wasmtime::Config;
    use wasmtime::Engine;
    use wasmtime::Store;
    use wit_component::ComponentEncoder;

    const GUEST_RS_WASM_MODULE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../guest-rs/target/wasm32-unknown-unknown/debug/guest.wasm"
    ));

    const IMAGE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/cat.png"));

    #[test]
    fn test_rs_guest() -> anyhow::Result<()> {
        let config = Config::new();
        let engine = Engine::new(&config)?;
        let mut store = Store::new(&engine, ());
        let linker = Linker::new(&engine);

        let component = ComponentEncoder::default()
            .module(GUEST_RS_WASM_MODULE)?
            .validate(true)
            .encode()?;
        let component = Component::from_binary(&engine, &component)?;

        // Instantiate the component and extract its functions
        let instance = linker.instantiate(&mut store, &component)?;
        let mut exports = instance.exports(&mut store);
        let mut intf = exports.instance("myintf").unwrap();

        let f0 = intf.func("extract-emails").unwrap();
        let f1 = intf.typed_func::<(String,), (Vec<String>,)>("extract-emails")?;
        let f2 = intf.typed_func::<(Vec<u8>,), (ResourceAny,)>("load-image")?;
        let f3 = intf.typed_func::<(ResourceAny, u32, u32), (ResourceAny,)>("resize-image")?;
        let f4 = intf.typed_func::<(ResourceAny,), (Vec<u8>,)>("image-to-bytes")?;
        drop(exports);

        // Dynamic call
        let emails0 = {
            let inputs = &[Val::String(
                "Hello my name is John Doe, my email is john.doe@gmail.com
                 I also have another email: john.doe@icloud.com
                 My friend's email is jane.doe@hotmail.com"
                    .to_owned()
                    .into_boxed_str(),
            )];
            let outputs = &mut [Val::Bool(false)];

            f0.call(&mut store, inputs, outputs)?;
            f0.post_return(&mut store)?;

            let Val::List(l) = &outputs.get(0).unwrap() else {
                panic!("unexpected type")
            };

            l.iter()
                .map(|v| {
                    let Val::String(s) = v else {
                        panic!("unexpected type")
                    };
                    s.to_string()
                })
                .collect::<Vec<String>>()
        };

        // Static call
        let (emails1,) = f1.call(
            &mut store,
            ("Hello my name is John Doe, my email is john.doe@gmail.com
              I also have another email: john.doe@icloud.com
              My friend's email is jane.doe@hotmail.com"
                .to_owned(),),
        )?;
        f1.post_return(&mut store)?;

        assert_eq!(emails0, emails1);

        // Opaque data types (resources)
        let (img,) = f2.call(&mut store, (IMAGE.to_vec(),))?;
        f2.post_return(&mut store)?;
        let (img,) = f3.call(&mut store, (img, 100, 100))?;
        f3.post_return(&mut store)?;
        let (bytes,) = f4.call(&mut store, (img,))?;
        f4.post_return(&mut store)?;

        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/cat_resized.png"),
            bytes,
        )?;

        println!("Image resized and saved to cat_resized.png");

        Ok(())
    }
}

#[cfg(test)]
mod wasm_wasi_test {
    use wasmtime::component::Component;
    use wasmtime::component::Linker;
    use wasmtime::component::ResourceTable;
    use wasmtime::Config;
    use wasmtime::Engine;
    use wasmtime::Store;
    use wit_component::ComponentEncoder;

    const GUEST_PY_WASI_COMPONENT: &[u8] =
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../guest-py/app.wasm"));

    const GUEST_RS_WASI_MODULE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../guest-rs/target/wasm32-wasi/debug/guest.wasm"
    ));

    struct Host {
        ctx: wasmtime_wasi::preview2::WasiCtx,
        table: ResourceTable,
    }

    impl wasmtime_wasi::preview2::WasiView for Host {
        fn table(&mut self) -> &mut ResourceTable {
            &mut self.table
        }

        fn ctx(&mut self) -> &mut wasmtime_wasi::preview2::WasiCtx {
            &mut self.ctx
        }
    }

    impl Host {
        fn new() -> Self {
            let ctx = wasmtime_wasi::preview2::WasiCtxBuilder::new().build();
            let table = ResourceTable::new();
            Self { ctx, table }
        }
    }

    #[test]
    fn test_rs_guest() -> anyhow::Result<()> {
        let mut config = Config::new();
        config.async_support(true);
        let engine = Engine::new(&config)?;
        let host = Host::new();
        let mut store = Store::new(&engine, host);
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::preview2::command::add_to_linker::<Host>(&mut linker)?;

        let component = ComponentEncoder::default()
            .module(GUEST_RS_WASI_MODULE)?
            .validate(true)
            .encode()?;
        let component = Component::from_binary(&engine, &component)?;
        let instance = linker.instantiate(&mut store, &component)?;
        let f1 = instance.get_typed_func::<(), (String,)>(&mut store, "hello")?;
        let (output,) = f1.call(&mut store, ())?;
        println!("Got output {}", output);
        Ok(())
    }

    #[test]
    fn test_py_guest() -> anyhow::Result<()> {
        let mut config = Config::new();
        // config.async_support(true);
        let engine = Engine::new(&config)?;
        let host = Host::new();
        let mut store = Store::new(&engine, host);
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::preview2::command::add_to_linker::<Host>(&mut linker)?;

        let component = Component::from_binary(&engine, &GUEST_PY_WASI_COMPONENT)?;
        let instance = linker.instantiate(&mut store, &component)?;
        let f1 = instance.get_typed_func::<(), (String,)>(&mut store, "hello")?;
        let (output,) = f1.call(&mut store, ())?;
        println!("Got output {}", output);

        Ok(())
    }
}
