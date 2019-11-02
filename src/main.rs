use std::fs::File;
use std::io;
use std::io::Read;
use std::env;
use xcb;

mod tga{
    use std::io::Read;

    #[derive(Debug)]
    pub enum TGAType{
        Original,
        New,
    }

    pub struct TGA{
        pub tga_type:               TGAType,
        pub id_length:              u8,
        pub cmap_type:              u8,
        pub image_type:             u8,
        pub cmap_spec_1ei:          u16,
        pub cmap_spec_length:       u16,
        pub cmap_spec_entry_size:   u8,
        pub image_spec_x_origin:    u16,
        pub image_spec_y_origin:    u16,
        pub image_spec_width:       u16,
        pub image_spec_height:      u16,
        pub image_spec_pixel_depth: u8,
        pub image_spec_descriptor:  u8,
        pub image_id:               Vec<u8>,
        pub cmap_data:              Vec<u8>,
        pub image_data:             Vec<u8>,
    }

    impl TGA {
        pub fn new(tga_type: TGAType,contents: Vec<u8>) -> TGA{
            let id_length              = contents[0];
            let image_type             = contents[2]; println!("im_type: {}",image_type);
            let cmap_spec_length       = ((contents[6] as u16) << 8) | contents[5] as u16; 
            let cmap_spec_entry_size   = contents[7];
            let image_spec_pixel_depth = contents[16];
            let image_spec_descriptor  = contents[17];
            let image_spec_width       = ((contents[13] as u16) << 8) | contents[12] as u16;
            let image_spec_height      = ((contents[15] as u16) << 8) | contents[14] as u16;
            let tag_id = 18 as usize;
            let tag_cmap = tag_id + id_length as usize;
            let tag_image = (tag_cmap as u16 + (cmap_spec_length * cmap_spec_entry_size as u16)/8) as usize;
            let tag_dev =  {let mut a = image_spec_descriptor<<4>>4;
                            if image_type == 2{a = 0;} //if TrueVista
                            println!("im_dscrptr_a: {}",a);
                            println!("im_height: {}",image_spec_height);
                            println!("im_width: {}",image_spec_width);
                            println!("im_depth: {}",image_spec_pixel_depth);
                            let bytepp:usize = ((image_spec_pixel_depth + a)/8) as usize;
                            println!("bytepp: {}",bytepp);
                            tag_image + (image_spec_height as usize * 
                            image_spec_width as usize * bytepp)};
            let tga = TGA{
                tga_type:               tga_type,
                id_length:              id_length,
                cmap_type:              contents[1],
                image_type:             image_type,
                cmap_spec_1ei:          ((contents[4] as u16) << 8) | contents[3] as u16,
                cmap_spec_length:       cmap_spec_length,
                cmap_spec_entry_size:   cmap_spec_entry_size,
                image_spec_x_origin:    ((contents[9] as u16) << 8) | contents[8] as u16,    
                image_spec_y_origin:    ((contents[11] as u16) << 8) | contents[10] as u16,
                image_spec_width:       image_spec_width,
                image_spec_height:      image_spec_height,
                image_spec_pixel_depth: image_spec_pixel_depth,
                image_spec_descriptor:  image_spec_descriptor,
                
                image_id:               {let mut a:Vec<u8> = Vec::new();
                                        (&contents[tag_id..tag_cmap]).read_to_end(&mut a).unwrap();
                                        a
                },
                cmap_data:              {let mut a:Vec<u8> = Vec::new();
                                        (&contents[tag_cmap..tag_image]).read_to_end(&mut a).unwrap();
                                        a
                },
                image_data:             {let mut a:Vec<u8> = Vec::new();
                                        println!("tag_image: {}\ntag_dev: {}\ncontents.len(): {}",tag_image,tag_dev,contents.len());
                                        (&contents[tag_image..tag_dev]).read_to_end(&mut a).unwrap();
                                        a
                },
            };
            tga
        }
    }

}
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len()<2{
        println!("Insert filename as first argument");
        return
    }
    let tga                = load_tga(&args[1]);
    
    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();
    let setup              = conn.get_setup();
    let screen             = setup.roots().nth(screen_num as usize).unwrap();
    
    let foreground         = conn.generate_id();
    xcb::create_gc(&conn, foreground, screen.root(), &[
            (xcb::GC_FOREGROUND, screen.black_pixel()),
            (xcb::GC_GRAPHICS_EXPOSURES, 0),
    ]);
    let win = conn.generate_id();
    xcb::create_window(&conn,
        xcb::COPY_FROM_PARENT as u8,
        win,
        screen.root(),
        0, 0,
        150, 150,
        10,
        xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
        screen.root_visual(), &[
            (xcb::CW_BACK_PIXEL, screen.white_pixel()),
            (xcb::CW_EVENT_MASK,
            xcb::EVENT_MASK_EXPOSURE | xcb::EVENT_MASK_KEY_PRESS),
        ]
    );
    let mut depth: u8 = 0;
    if (tga.image_type == 2){
        depth = tga.image_spec_pixel_depth;
    }
    else{
        println!("Not TrueColor.");
        return
    }

    let (width,height) = (tga.image_spec_width,tga.image_spec_height);
    let pixmap = conn.generate_id();
    xcb::create_pixmap(&conn, depth, pixmap,win,width,height);  //Creating a pixmap
    xcb::put_image(&conn,
                   xcb::IMAGE_FORMAT_XY_BITMAP as u8,
                   pixmap,
                   foreground,
                   width,
                   height,
                   0,
                   0,
                   0,
                   depth,
                   &tga.image_data
    );

    xcb::map_window(&conn, win);
    conn.flush();
    loop {
        let event = conn.wait_for_event();
        match event {
            None => { break; }
            Some(event) => {
                let r = event.response_type() & !0x80;
                match r {
                    xcb::EXPOSE => {
                        /* We draw */
                        xcb::copy_area(&conn,
                                       pixmap,
                                       win,
                                       foreground,
                                       0,0,0,0,
                                       width,height
                        );
                        /* We flush the request */
                        conn.flush();

                    },
                    xcb::KEY_PRESS => {
                        let key_press : &xcb::KeyPressEvent = unsafe {
                            xcb::cast_event(&event)
                        };

                        println!("Key '{}' pressed", key_press.detail());
                        if key_press.detail() == 9{
                            break;
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}

fn load_tga(filepath:&String) -> tga::TGA {
    let mut file = File::open(filepath).unwrap();
    let mut contents:Vec<u8> = Vec::new();
    let mut footer:Vec<u8> = Vec::new();
    file.read_to_end(&mut contents).unwrap();
    (&contents[(contents.len()-26)..contents.len()]).read_to_end(&mut footer).unwrap(); 
    
    let mut s:Vec<u8> = Vec::new();
    (&footer[8..=23]).read_to_end(&mut s).unwrap();
    let s:String = String::from_utf8(s).unwrap();
    
    let tgatype = match &s as &str {
        "TRUEVISION-XFILE" => tga::TGAType::New,
                        _ => tga::TGAType::Original,
    };
    println!("Type:{:?}, footer:'{}'",&tgatype,&s);
    tga::TGA::new(tgatype,contents)
    //let field_color_map_spec;
    //&contents[3..8].read(&mut field_color_map_spec)?;
}
