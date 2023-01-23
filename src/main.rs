use btleplug::api::{
    bleuuid::uuid_from_u16, Central, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use rand::{thread_rng, Rng};
use std::error::Error;
use std::time::Duration;
use uuid::{uuid, Uuid};
use futures::stream::StreamExt;

const CONTROL_CHARACTERISTIC_UUID: Uuid = uuid!("f000fff5-0451-4000-b000-000000000000");
const NOTIFY_CHARACTERISTIC_UUID: Uuid = uuid!("f000fff4-0451-4000-b000-000000000000");
use tokio::time;
use tokio::sync::mpsc;

use std::fs::File;
use std::io::prelude::*;


const CRC_TABLE: [u16; 256] = [0, 4129, 8258, 12387, 16516, 20645, 24774, 28903, 33032, 37161, 41290, 45419, 49548, 53677, 57806, 61935, 4657, 528, 12915, 8786, 21173, 17044, 29431, 25302, 37689, 33560, 45947, 41818, 54205, 50076, 62463, 58334, 9314, 13379, 1056, 5121, 25830, 29895, 17572, 21637, 42346, 46411, 34088, 38153, 58862, 62927, 50604, 54669, 13907, 9842, 5649, 1584, 30423, 26358, 22165, 18100, 46939, 42874, 38681, 34616, 63455, 59390, 55197, 51132, 18628, 22757, 26758, 30887, 2112, 6241, 10242, 14371, 51660, 55789, 59790, 63919, 35144, 39273, 43274, 47403, 23285, 19156, 31415, 27286, 6769, 2640, 14899, 10770, 56317, 52188, 64447, 60318, 39801, 35672, 47931, 43802, 27814, 31879, 19684, 23749, 11298, 15363, 3168, 7233, 60846, 64911, 52716, 56781, 44330, 48395, 36200, 40265, 32407, 28342, 24277, 20212, 15891, 11826, 7761, 3696, 65439, 61374, 57309, 53244, 48923, 44858, 40793, 36728, 37256, 33193, 45514, 41451, 53516, 49453, 61774, 57711, 4224, 161, 12482, 8419, 20484, 16421, 28742, 24679, 33721, 37784, 41979, 46042, 49981, 54044, 58239, 62302, 689, 4752, 8947, 13010, 16949, 21012, 25207, 29270, 46570, 42443, 38312, 34185, 62830, 58703, 54572, 50445, 13538, 9411, 5280, 1153, 29798, 25671, 21540, 17413, 42971, 47098, 34713, 38840, 59231, 63358, 50973, 55100, 9939, 14066, 1681, 5808, 26199, 30326, 17941, 22068, 55628, 51565, 63758, 59695, 39368, 35305, 47498, 43435, 22596, 18533, 30726, 26663, 6336, 2273, 14466, 10403, 52093, 56156, 60223, 64286, 35833, 39896, 43963, 48026, 19061, 23124, 27191, 31254, 2801, 6864, 10931, 14994, 64814, 60687, 56684, 52557, 48554, 44427, 40424, 36297, 31782, 27655, 23652, 19525, 15522, 11395, 7392, 3265, 61215, 65342, 53085, 57212, 44955, 49082, 36825, 40952, 28183, 32310, 20053, 24180, 11923, 16050, 3793, 7920];
const PACKETS_IN_BLOCK: usize = 206;
const BLOCK_SIZE: usize = 4096;
const CRC_INDEX: usize = (PACKETS_IN_BLOCK - 1) * 20; // equals BLOCK_SIZE + 4

fn compute_crc(data: &[u8]) -> [u8; 2] {
    let mut crc_result: u16 = 0;
    for byte in data.iter() {
        crc_result = (crc_result << 8) ^ CRC_TABLE[((crc_result >> 8) ^ (*byte as u16)) as usize];
    }
    [(crc_result >> 8) as u8, crc_result as u8]
}

#[derive(Debug)]
enum Command {
    Reset,
    SendTime,
    GetBattery,
    GetTime,
    GetVersion,
    Test,
    GetAddress,
    ModeFunc,
    ModeFuncState,
    PhoneSwitch,
    GetCalInfo,
    GetHistory,
    ClearHistory,
    GetSedentaryTime,
    SetAlarm,
    SetUserInfo,
    SetRightHand,
    ForceSleep,
    NewPairing,
    GetData,
    GetDataInfo,
    GetDataFinish,
}

fn pack_command(base: &[u8]) -> [u8; 20] {
    let mut command: [u8; 20] = [255; 20];
    command[0] = 126;
    command[1..1+base.len()].copy_from_slice(&base);
    let crc = compute_crc(&command[1..18]);
    command[18..20].copy_from_slice(&crc);
    command
}

fn gen_command(command_type: Command) -> [u8; 20] {
    let raw: &[u8] = match command_type {
        Command::Reset=>&[19, 0],
        Command::GetBattery=>&[20],
        Command::GetTime=>&[17],
        Command::NewPairing=>&[65, 160, 161],
        Command::GetVersion=>&[18],
        Command::GetHistory=>&[35],
        Command::GetDataInfo=>&[21],
        _=>panic!("Invalid command type!"),
    };
    pack_command(&raw)
}

async fn find_tracker(central: &Adapter) -> Option<Peripheral> {
    for p in central.peripherals().await.unwrap() {
        if p.properties()
            .await
            .unwrap()
            .unwrap()
            .local_name
            .iter()
            .any(|name| name.contains("DSB012"))
        {
            return Some(p);
        }
    }
    None
}

#[derive(Debug, PartialEq, Eq)]
enum Response {
    PairOk,
    FwVersion,
    DataInfo,
    DataFinishOk,
}

async fn parse_response(data: &[u8]) -> Response {
    let response: [u8; 20] = data.try_into().unwrap();
    if response[0] != 126 {
        panic!("Invalid command prefix!")
    };

    let crc = compute_crc(&response[1..18]);
    if response[18] != crc[0] || response[19] != crc[1] {
        panic!("CRC mismatch! r:{:?} c:{:?}", &response[18..20], crc);
    }

    match response[1] {
        73 => Response::PairOk,
        2 => Response::FwVersion,
        5 => panic!("Tried to parse data info as single packet"),
        6 => panic!("Tried to parse data block as single packet"),
        7 => Response::DataFinishOk,
        _ => panic!("Invalid / unimplemented response! r:{:?}", response)
    }
}

async fn parse_block(data: &[u8]) -> &[u8] {
    if data[0] != 126 || data[1] != 6 {
        panic!("Invalid data block header!");
    }

    let crc = &data[CRC_INDEX..CRC_INDEX+2];

    let real_crc = compute_crc(&data[1..CRC_INDEX]);

    if crc[0] != real_crc[0] || crc[1] != real_crc[1] {
        panic!("CRC mismatch! r:{:?} c:{:?}", crc, real_crc); 
    }

    &data[4..CRC_INDEX]
}

struct DataInfo {
    DataStart: u16,
    DataEnd: u16,
    FlashSize: u16,
}

async fn parse_data_info(data: &[u8]) -> DataInfo {
    if data[0] != 126 || data[1] != 5 {
        panic!("Invalid data info header!");
    }

    let crc = compute_crc(&data[1..18]);
    if data[18] != crc[0] || data[19] != crc[1] {
        panic!("CRC mismatch! r:{:?} c:{:?}", &data[18..20], crc);
    }

    DataInfo {
        DataStart: ((data[2] as u16) << 8) | data[3] as u16,
        DataEnd: ((data[4] as u16) << 8) | data[5] as u16,
        FlashSize: ((data[6] as u16) << 8) | data[7] as u16,
    }
}

fn gen_data_command(start: u16, file: u16) -> [u8; 20] {
    pack_command(&[22, (start  >> 8) as u8, start as u8, (file >> 8) as u8, file as u8])
}

fn gen_data_finish_command(start: u16, file: u16) -> [u8; 20] {
    pack_command(&[23, (start  >> 8) as u8, start as u8, (file >> 8) as u8, file as u8])
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    Pairing,
    Connected,
    Ready,
    Receiving,
    Disconnected,
}

#[derive(Debug)]
enum TaskMsg {
    StateChange (State),
    Response (ResponseData),
}

#[derive(Debug)]
struct ResponseData {

}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await.unwrap();

    let adapter = manager
        .adapters()
        .await
        .expect("Unable to fetch adapter list.")
        .into_iter()
        .nth(0)
        .expect("Unable to find adapters.");

    adapter.start_scan(ScanFilter::default()).await?;
    time::sleep(Duration::from_secs(2)).await;

    let tracker = find_tracker(&adapter).await.expect("No trackers found");

    tracker.connect().await?;

    let mut stream = tracker.notifications().await?;

    let (tx, mut rx) = mpsc::channel(2);

    tokio::spawn(async move {
        tx.send(TaskMsg::StateChange(State::Pairing)).await.unwrap();

        tracker.discover_services().await.unwrap();

        let chars = tracker.characteristics();

        let tx_char = chars
            .iter()
            .find(|c| c.uuid == CONTROL_CHARACTERISTIC_UUID)
            .expect("Unable to find tx characteric");

        let rx_char = chars
            .iter()
            .find(|c| c.uuid == NOTIFY_CHARACTERISTIC_UUID)
            .expect("Unable to find rx characteric");

        tracker.subscribe(&rx_char).await.unwrap();

        let mut stream = tracker.notifications().await.unwrap();

        tx.send(TaskMsg::StateChange(State::Connected)).await.unwrap();

        tracker.write(&tx_char, &gen_command(Command::GetVersion), WriteType::WithResponse).await.unwrap();

        let data = stream.next().await.unwrap();
        let response = parse_response(&data.value).await;

        if response != Response::FwVersion {
            panic!("Pair request failed!");
        }

        println!("Got FW version: {:?}", &data.value[2..18]);
        //std::str::from_utf8(&data.value[2..18]).unwrap()

        tracker.write(&tx_char, &gen_command(Command::NewPairing), WriteType::WithResponse).await.unwrap();

        let data = stream.next().await.unwrap();
        let response = parse_response(&data.value).await;

        if response != Response::PairOk {
            panic!("Pair request failed!");
        }

        tx.send(TaskMsg::StateChange(State::Ready)).await.unwrap();

        tracker.write(&tx_char, &gen_command(Command::GetDataInfo), WriteType::WithResponse).await.unwrap();

        let data = stream.next().await.unwrap();
        println!("i {:?}", data.value);
        let data_info = parse_data_info(&data.value).await;

        if data_info.DataStart != 0 || data_info.DataEnd > data_info.FlashSize {
            panic!("Unsupported data ranges! start: {:?} end: {:?}", data_info.DataStart, data_info.DataEnd);
        }

        tx.send(TaskMsg::StateChange(State::Receiving)).await.unwrap();

        let block_count = data_info.DataEnd;

        let mut flash = Vec::new(); 
        for block_index in 0..block_count {
            tracker.write(&tx_char, &gen_data_command(block_index as u16, 1 as u16), WriteType::WithResponse).await.unwrap();
            let mut block = Vec::new();
            for packet_index in 0..PACKETS_IN_BLOCK {
                let packet = stream.next().await.unwrap();
                block.extend(packet.value.iter().copied());
                //println!("d {:?}/{:?} {:?}", packet_index+1, PACKETS_IN_BLOCK, packet.value);
                println!("Pulling data... {:?}%", ((packet_index + PACKETS_IN_BLOCK * (block_index as usize)) * 100) / (PACKETS_IN_BLOCK * block_count as usize));
            };
            //println!("Block {:?}/{:?} received", block_index+1, block_count);
            flash.extend(parse_block(&block).await.iter().copied());
        };

        let mut file = File::create("flash.bin").unwrap();
        file.write_all(&flash);

        //tracker.write(&tx_char, &gen_data_finish_command(data_info.DataStart, data_info.DataEnd), WriteType::WithResponse).await.unwrap();

        //let data = stream.next().await.unwrap();
        //let response = parse_response(&data.value).await;

        //if response != Response::DataFinishOk || data.value[2] != 1 {
        //    panic!("Data finish failed!");
        //};

        tx.send(TaskMsg::StateChange(State::Ready)).await.unwrap();
        
        //tracker.write(&tx_char, &pack_command(&[34]), WriteType::WithResponse).await.unwrap();

        //while let Some(packet) = stream.next().await {
        //    println!("d {:?}", packet.value);
        //}
    });

    while let Some(message) = rx.recv().await {
        match message {
            TaskMsg::StateChange (state) => match state {
                State::Pairing => println!("Pairing..."),
                State::Connected => println!("Connected..."),
                State::Ready => println!("Tracker is Ready"),
                State::Receiving => println!("State: Receiving"),
                State::Disconnected => println!("State: Disconnected"),
            },
            TaskMsg::Response (response) => {
                panic!("Unimplemented!");
            }
        }
    }

    Ok(())
}

// 17
// 18
// 20
