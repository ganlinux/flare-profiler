use std::fs::{File, OpenOptions};
use std::io::{SeekFrom, ErrorKind, Seek, Write, Read, BufReader};
use chrono::Local;
use std::io::Error;
use std::io;
use byteorder::{WriteBytesExt, ReadBytesExt, NetworkEndian};
use std::str::from_utf8;
use num::FromPrimitive;
use std::cmp::*;

use super::FileEndian;

#[derive(Clone, PartialEq)]
pub enum TSValue {
    int16(i16),
    int32(i32),
    int64(i64)
}

#[derive(Clone, PartialEq, Debug)]
pub enum TSRangeValue {
    vec_int16(Vec<i16>),
    vec_int32(Vec<i32>),
    vec_int64(Vec<i64>),
    vec_f32(Vec<f32>),
}

#[derive(Debug)]
pub struct TSResult {
    pub begin_time: i64,
    pub end_time: i64,
    pub unit_time: i32,
    pub steps: i32,
    pub data: TSRangeValue
}

enum_from_primitive! {
    #[derive(Clone, Copy, PartialEq, Debug)]
    pub enum ValueType {
        UNKNOWN,
        INT16,
        INT32,
        INT64
    }
}

pub fn get_unit_len(value_type: ValueType) -> i8{
    match value_type {
        INT16 => 2,
        INT32 => 4,
        INT64 => 8
    }
}

#[derive( Debug )]
pub struct TimeSeriesFile {
    //file path
    pub path: String,
    //file object
    file: File,
    //data segment start offset
    pub data_offset: u64,

    // value data type
    pub value_type: ValueType,
    // value unit len(bytes)
    pub unit_len: i8,
    //sample unit time ms
    pub unit_time: i32,
    // data start time ms
    pub begin_time: i64,
    // current time ms
    pub end_time: i64,
    // sample count
    pub amount: i32,
}

pub struct TimeSeriesFileReader {
    info: TimeSeriesFile,
    inited: bool,

}

pub struct TimeSeriesFileWriter {
    info: TimeSeriesFile,
    inited: bool,

}

impl TimeSeriesFile {

    fn new(value_type: ValueType, unit_time: i32, path: &str, file: File) -> TimeSeriesFile {
        TimeSeriesFile{
            path: path.to_string(),
            file: file,
            data_offset: 0,
            unit_time: unit_time,
            unit_len: get_unit_len(value_type),
            value_type: value_type,
            begin_time: 0,
            end_time: 0,
            amount: 0
        }
    }

    pub fn get_range_value_int16(&self, start_time: i64, end_time: i64, unit_time_ms: i32) -> TSResult {
        let mut data_vec = vec![];
        // self.begin_time <= start_time <= self.end_time
        let mut start_time = max(self.begin_time, start_time);
        start_time = min(start_time, self.end_time);

        // self.begin_time <= end_time <= self.end_time
        let mut end_time = max(self.begin_time, end_time);
        end_time = min(end_time, self.end_time);

        //convert time to steps
        let step1 = (start_time - self.begin_time) / self.unit_time as i64;
        let step2 = (end_time - self.begin_time) / self.unit_time as i64;

        //convert steps to file offset
        let offset1 = self.data_offset + self.unit_len as u64 * step1 as u64;
        let offset2 = self.data_offset + self.unit_len as u64 * step2 as u64;

        //read specify range data
        let mut file = &self.file;
        file.seek(SeekFrom::Start(offset1));
        //let mut stream = file.take(offset2 - offset1);
        //stream.read_i16_into::<FileEndian>(data_vec.as_mut_slice());
        let bytes = offset2 - offset1;
        let mut buf_reader = BufReader::with_capacity(1024*100, file);
        let steps = bytes / self.unit_len as u64;
        for i in 0..steps {
            data_vec.push(buf_reader.read_i16::<FileEndian>().unwrap());
        }

        //convert time unit, merge n source point to one new point
        let merge_num = (unit_time_ms / self.unit_time) as usize;
        let mut unit_time_ms = merge_num as i32 * self.unit_time;
        if merge_num > 1 {
            let mut new_data_vec = vec![];
            let size = data_vec.len() / merge_num as usize;
            for i in 0..size {
                new_data_vec.push(ts_avg_int16(&data_vec[i*merge_num..(i+1)*merge_num]));
            }
            TSResult {
                begin_time: start_time,
                end_time,
                unit_time: unit_time_ms,
                steps: new_data_vec.len() as i32,
                data: TSRangeValue::vec_f32(new_data_vec)
            }
        }else {
            TSResult {
                begin_time: start_time,
                end_time,
                unit_time: unit_time_ms,
                steps: data_vec.len() as i32,
                data: TSRangeValue::vec_int16(data_vec)
            }
        }
    }

//    pub fn get_range_value_int32(&mut self, start_time: i64, end_time: i64, unit_time_ms: i32) -> Vec<i32> {
//
//    }
}

fn ts_avg_int16 (numbers: &[i16]) -> f32 {
    let mut sum = 0;
    numbers.iter().for_each(|x| sum += *x as i32);
    sum as f32 / numbers.len() as f32
}


//fn average(numbers: &[i32]) -> f32 {
//    numbers.iter().sum::<i32>() as f32 / numbers.len() as f32
//}
//
//fn median(numbers: &mut [i32]) -> i32 {
//    numbers.sort();
//    let mid = numbers.len() / 2;
//    numbers[mid]
//}


impl TimeSeriesFileReader {

    pub fn new(path: &str) -> Result<TimeSeriesFileReader, Error> {
        //let now_time = Local::now().timestamp_millis();
        match File::open(path.clone()) {
            Ok(file) => {
                let info = TimeSeriesFile::new(ValueType::UNKNOWN, 0, path, file);
                let mut reader = TimeSeriesFileReader {
                    info: info,
                    inited: false
                };
                reader.init();
                Ok(reader)
            },
            Err(err) => Err(err)
        }
    }

    fn init(&mut self) -> Result<bool, Error> {
        if !self.inited {
            let info = &mut self.info;

            //read file header
            let mut flag_buf = [0 as u8;4];
            let file = &mut info.file;
            file.seek(SeekFrom::Start(0));
            //TS file header segment: TSHS (4 bytes)
            file.read_exact(&mut flag_buf[..]);
            let flag = std::str::from_utf8(&flag_buf[..]).unwrap();
            if flag != "TSHS" {
                println!("Invalid time series file, header segment flag not match");
                return Err(io::Error::new(ErrorKind::InvalidInput, "Invalid time series file (header segment)"));
            }

            //header len (2 bytes)
            let header_len = file.read_u16::<FileEndian>().unwrap() as u64;
            let header_offset = 4 + 2;

            //header data (n bytes)
            info.value_type = ValueType::from_i8(file.read_i8().unwrap()).unwrap();
            info.unit_len = file.read_i8().unwrap();
            info.unit_time = file.read_i32::<FileEndian>().unwrap();
            info.begin_time = file.read_i64::<FileEndian>().unwrap();
            info.end_time = file.read_i64::<FileEndian>().unwrap();
            info.amount = file.read_i32::<FileEndian>().unwrap();

            //data segment flag
            file.seek(SeekFrom::Start(header_offset + header_len));
            file.read_exact(&mut flag_buf[..]);
            let flag = std::str::from_utf8(&flag_buf[..]).unwrap();
            if flag != "TSDS" {
                println!("Invalid time series file, data segment flag not match: {}", flag);
                return Err(io::Error::new(ErrorKind::InvalidInput, "Invalid time series file (data segment)"));
            }

            //save data segment start offset
            info.data_offset = header_offset + header_len + 4;

            self.inited = true;
        }
        Ok(true)
    }

    pub fn get_header_info(&self) -> &TimeSeriesFile {
        &self.info
    }

    pub fn get_range_value_int16(&self, start_time: i64, end_time: i64, unit_time_ms: i32) -> TSResult {
        self.info.get_range_value_int16(start_time, end_time, unit_time_ms)
    }

}


impl TimeSeriesFileWriter {

    pub fn new(value_type: ValueType, unit_time: i32, path: &str) -> Result<TimeSeriesFileWriter, Error> {
        let now_time = Local::now().timestamp_millis();
        let file_rs = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path.clone());
        match file_rs {
            Ok(file) => {
                let info = TimeSeriesFile::new(value_type, unit_time, path, file);
                Ok(TimeSeriesFileWriter {
                    info: info,
                    inited: false
                })
            },
            Err(err) => Err(err)
        }
    }

    fn init(&mut self, time: i64) -> Result<bool, Error> {

        if !self.inited {
            let info = &mut self.info;
            info.begin_time = time;
            info.end_time = time;

            self.save_header_info();

            self.inited = true;
        }
        Ok(true)
    }

    fn save_header_info(&mut self) {
        let info = &mut self.info;
        //encode header
        let mut header_vec = vec![];
        header_vec.write_i8(info.value_type as i8);
        header_vec.write_i8(info.unit_len);
        header_vec.write_i32::<FileEndian>(info.unit_time);
        header_vec.write_i64::<FileEndian>(info.begin_time);
        header_vec.write_i64::<FileEndian>(info.end_time);
        header_vec.write_i32::<FileEndian>(info.amount);

        //write file header
        let file = &mut info.file;
        file.seek(SeekFrom::Start(0));

        //TS file header segment: TSHS (4 bytes)
        file.write_all(b"TSHS");

        //header len (2 bytes)
        file.write_u16::<FileEndian>(header_vec.len() as u16);

        //header data (n bytes)
        file.write_all(header_vec.as_slice());

        //data segment flag: TSDS
        file.write_all(b"TSDS");

        //save data segment start offset
        info.data_offset = file.seek(SeekFrom::Current(0)).unwrap();
        //info.data_offset = file.stream_position().unwrap();

    }

    pub fn get_header_info(&mut self) -> &TimeSeriesFile {
        &self.info
    }

    pub fn add_value(&mut self, time: i64, value: TSValue) -> Result<bool, Error> {
        self.init(time);
        let info = &mut self.info;
        let steps = (time - info.begin_time) / info.unit_time as i64;
        let file = &mut info.file;
        info.amount = min(info.amount+1, steps as i32 +1);
        info.end_time = info.begin_time + steps * info.unit_time as i64;

        match value {
            TSValue::int16(val) => {
                if info.value_type != ValueType::INT16 {
                    println!("value type not match, expect {:?} but {:?}", info.value_type, ValueType::INT16);
                    return Err(io::Error::new(ErrorKind::InvalidInput, "value type not match"));
                }
                let offset = (steps * 2) as u64;
                file.seek(SeekFrom::Start(info.data_offset + offset));
                file.write_i16::<FileEndian>(val);
            }
            TSValue::int32(val) => {
                if info.value_type != ValueType::INT32 {
                    println!("value type not match, expect {:?} but {:?}", info.value_type, ValueType::INT32);
                    return Err(io::Error::new(ErrorKind::InvalidInput, "value type not match"));
                }
                let offset = (steps * 4) as u64;
                file.seek(SeekFrom::Start(info.data_offset + offset));
                file.write_i32::<FileEndian>(val);
            }
            TSValue::int64(val) => {
                if info.value_type != ValueType::INT64 {
                    println!("value type not match, expect {:?} but {:?}", info.value_type, ValueType::INT64);
                    return Err(io::Error::new(ErrorKind::InvalidInput, "value type not match"));
                }
                let offset = (steps * 8) as u64;
                file.seek(SeekFrom::Start(info.data_offset + offset));
                file.write_i64::<FileEndian>(val);
            }
            _ => {
                println!("unsupported value type: {:?}", info.value_type);
                return Err(io::Error::new(ErrorKind::InvalidInput, "unsupported value type"));
            }
        }

        Ok(true)
    }

    pub fn get_range_value_int16(&self, start_time: i64, end_time: i64, unit_time_ms: i32) -> TSResult {
        self.info.get_range_value_int16(start_time, end_time, unit_time_ms)
    }

}

impl Drop for TimeSeriesFileWriter {

    fn drop(&mut self) {
        self.save_header_info();
    }

}