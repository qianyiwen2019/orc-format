use std::io::{Write, Result};

use crate::protos::orc_proto;
use crate::schema::Schema;
use crate::writer::Config;
use crate::writer::count_write::CountWrite;
use crate::writer::compression::CompressionStream;
use crate::writer::encoder::{BooleanRLE, SignedIntRLEv1, VarInt};
use crate::writer::stripe::StreamInfo;
use crate::writer::statistics::{Statistics, BaseStatistics, Decimal64Statistics};
use crate::writer::data::common::BaseData;


pub struct Decimal64Data<'a> {
    pub(crate) column_id: u32,
    pub(crate) schema: &'a Schema,
    precision: u32,
    scale: u32,
    present: BooleanRLE,
    data: CompressionStream,

    // Only the constant 'scale' is repeatedly written to this. It is only included to satisfy 
    // ORC v1 spec (should no longer be needed in ORC v2).
    secondary_scale: SignedIntRLEv1,  

    stripe_stats: Decimal64Statistics,
}

impl<'a> Decimal64Data<'a> {
    pub(crate) fn new(schema: &'a Schema, config: &'a Config, column_id: &mut u32) -> Self {
        let cid = *column_id;
        *column_id += 1;
        if let Schema::Decimal(precision, scale) = schema {
            Self {
                column_id: cid,
                schema,
                precision: *precision,
                scale: *scale,
                present: BooleanRLE::new(&config.compression),
                data: CompressionStream::new(&config.compression),
                secondary_scale: SignedIntRLEv1::new(&config.compression),
                stripe_stats: Decimal64Statistics::new(*scale),
            }
        } else { unreachable!() }
    }

    pub fn write(&mut self, x: Option<i64>) {
        match x {
            Some(y) => {
                self.present.write(true);
                y.write_varint(&mut self.data);
                self.secondary_scale.write(self.scale as i64);
            }
            None => { 
                self.present.write(false); 
            }
        }
        self.stripe_stats.update(x);
    }

    pub fn precision(&self) -> u32 { self.precision }

    pub fn scale(&self) -> u32 { self.scale }
}

impl<'a> BaseData<'a> for Decimal64Data<'a> {
    fn schema(&self) -> &'a Schema { self.schema }

    fn column_id(&self) -> u32 { self.column_id }

    fn write_index_streams<W: Write>(&mut self, _out: &mut CountWrite<W>, _stream_infos_out: &mut Vec<StreamInfo>) -> Result<()> {
        Ok(())
    }

    fn write_data_streams<W: Write>(&mut self, out: &mut CountWrite<W>, stream_infos_out: &mut Vec<StreamInfo>) -> Result<()> {
        if self.stripe_stats.has_null() {
            let present_start_pos = out.pos();
            self.present.finish(out)?;
            let present_len = (out.pos() - present_start_pos) as u64;
            stream_infos_out.push(StreamInfo {
                kind: orc_proto::Stream_Kind::PRESENT,
                column_id: self.column_id,
                length: present_len,
            });
        }

        let data_start_pos = out.pos();
        self.data.finish(out)?;
        let data_len = (out.pos() - data_start_pos) as u64;
        stream_infos_out.push(StreamInfo {
            kind: orc_proto::Stream_Kind::DATA,
            column_id: self.column_id,
            length: data_len,
        });

        let secondary_start_pos = out.pos();
        self.secondary_scale.finish(out)?;
        let secondary_len = (out.pos() - secondary_start_pos) as u64;
        stream_infos_out.push(StreamInfo {
            kind: orc_proto::Stream_Kind::SECONDARY,
            column_id: self.column_id,
            length: secondary_len,
        });

        Ok(())
    }

    fn column_encodings(&self, out: &mut Vec<orc_proto::ColumnEncoding>) {
        let mut encoding = orc_proto::ColumnEncoding::new();
        encoding.set_kind(orc_proto::ColumnEncoding_Kind::DIRECT);
        out.push(encoding);
    }

    fn statistics(&self, out: &mut Vec<Statistics>) {
        out.push(Statistics::Decimal64(self.stripe_stats));
    }

    fn estimated_size(&self) -> usize {
        self.present.estimated_size() + self.data.estimated_size()
    }

    fn verify_row_count(&self, expected_row_count: u64) {
        let rows_written = self.stripe_stats.num_values();
        if rows_written != expected_row_count {
            panic!("In column {} (type Decimal64), the number of values written ({}) does not match the expected number ({})", 
                self.column_id, rows_written, expected_row_count);
        }
    }

    fn reset(&mut self) {
        self.stripe_stats = Decimal64Statistics::new(self.scale);
    }
}