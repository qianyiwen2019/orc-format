use std::io::{Write, Result};

use crate::protos::orc_proto;
use crate::schema::{Schema, Field};
use crate::writer::Config;
use crate::writer::count_write::CountWrite;
use crate::writer::encoder::BooleanRLE;
use crate::writer::stripe::StreamInfo;
use crate::writer::statistics::{Statistics, BaseStatistics, GenericStatistics};
use crate::writer::data::common::BaseData;
use crate::writer::data::Data;

pub struct StructData<'a> {
    column_id: u32,
    pub(crate) schema: &'a Schema,
    pub(crate) children: Vec<Data<'a>>,
    present: BooleanRLE,
    stripe_stats: GenericStatistics,
}

impl<'a> StructData<'a> {
    pub(crate) fn new(schema: &'a Schema, config: &'a Config, column_id: &mut u32) -> Self {
        let cid = *column_id;
        let mut children: Vec<Data> = Vec::new();
        *column_id += 1;

        if let Schema::Struct(fields) = schema {
            for field in fields {
                children.push(Data::new(&field.1, config, column_id));
            }

            StructData {
                column_id: cid,
                schema,
                present: BooleanRLE::new(&config.compression),
                children: children,
                stripe_stats: GenericStatistics::new(),
            }
        } else { unreachable!() }
    }
    
    pub fn fields(&self) -> &'a [Field] {
        if let Schema::Struct(f) = self.schema {
            f
        } else { unreachable!() }
    }

    pub fn children(&mut self) -> &mut [Data<'a>] {
        &mut self.children
    }

    pub fn child(&mut self, i: usize) -> &mut Data<'a> {
        &mut self.children[i]
    }

    pub fn write(&mut self, present: bool) {
        self.present.write(present);
        self.stripe_stats.update(present);
    }

    pub fn column_id(&self) -> u32 { self.column_id }
}

impl<'a> BaseData<'a> for StructData<'a> {
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
        
        for child in &mut self.children {
            child.write_data_streams(out, stream_infos_out)?;
        }

        Ok(())
    }

    fn column_encodings(&self, out: &mut Vec<orc_proto::ColumnEncoding>) {
        assert_eq!(out.len(), self.column_id as usize);
        let mut encoding = orc_proto::ColumnEncoding::new();
        encoding.set_kind(orc_proto::ColumnEncoding_Kind::DIRECT);
        out.push(encoding);
        for child in &self.children {
            child.column_encodings(out);
        }
    }

    fn statistics(&self, out: &mut Vec<Statistics>) {
        assert_eq!(out.len(), self.column_id as usize);
        out.push(Statistics::Generic(self.stripe_stats));
        for child in &self.children {
            child.statistics(out);
        }
    }

    fn verify_row_count(&self, expected_row_count: u64) {
        let rows_written = self.stripe_stats.num_values();
        if rows_written != expected_row_count {
            panic!("In column {} (type Struct), the number of values written ({}) does not match the expected number ({})", 
                self.column_id, rows_written, expected_row_count);
        }

        for child in &self.children {
            child.verify_row_count(self.stripe_stats.num_present());
        }
    }

    fn estimated_size(&self) -> usize {
        let mut size = self.present.estimated_size();
        for child in &self.children {
            size += child.estimated_size();
        }
        size
    }

    fn reset(&mut self) {
        self.stripe_stats = GenericStatistics::new();
        for child in &mut self.children {
            child.reset();
        }
    }
}
