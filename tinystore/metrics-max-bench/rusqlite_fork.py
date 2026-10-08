#!/usr/bin/env python3
"""Build three local rusqlite patches, retaining minimal diffs and hashes."""
import difflib
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import tomllib

ROOT=Path(__file__).resolve().parent
WORK=Path(os.environ.get('ADAPTER_WORK','/work'))
DATA=ROOT.parent/'reports/data/rusqlite-fork-2026-10-08'

def replace(text,old,new):
    assert text.count(old)==1,old
    return text.replace(old,new)

def patch(kind,source):
    text={name:(source/'src'/name).read_text() for name in ['raw_statement.rs','statement.rs']}
    if kind in ['inline','combined']:
        text['statement.rs']=replace(text['statement.rs'],
            '    pub(super) fn value_ref(&self, col: usize) -> ValueRef',
            '    #[inline]\n    pub(super) fn value_ref(&self, col: usize) -> ValueRef')
    if kind in ['count','combined']:
        value=text['raw_statement.rs']
        value=replace(value,'use std::ptr;','use std::ptr;\nuse std::cell::Cell;')
        value=replace(value,'pub struct RawStatement {\n    ptr: *mut ffi::sqlite3_stmt,',
            'pub struct RawStatement {\n    ptr: *mut ffi::sqlite3_stmt,\n    row_column_count: Cell<Option<usize>>,')
        value=replace(value,'            ptr: stmt,',
            '            ptr: stmt,\n            row_column_count: Cell::new(None),')
        value=replace(value,
            '        // Note: Can\'t cache this as it changes if the schema is altered.\n        unsafe { ffi::sqlite3_column_count(self.ptr) as usize }',
            '''        // Cache only within one step/reset epoch. A new step may reprepare
        // SELECT * after a schema change, so it invalidates this value first.
        if let Some(count) = self.row_column_count.get() { return count; }
        let count = unsafe { ffi::sqlite3_column_count(self.ptr) as usize };
        self.row_column_count.set(Some(count));
        count''')
        value=replace(value,'    pub fn step(&self) -> c_int {',
            '    pub fn step(&self) -> c_int {\n        self.row_column_count.set(None);')
        value=replace(value,'    pub fn reset(&self) -> c_int {',
            '    pub fn reset(&self) -> c_int {\n        self.row_column_count.set(None);')
        text['raw_statement.rs']=value
    return text

TESTS='''
#[cfg(test)]
mod tinystore_count_cache_regressions {
    use crate::{Connection, Error, Result};
    #[test]
    fn schema_reprepare_refreshes_count_and_keeps_invalid_index_errors() -> Result<()> {
        let conn=Connection::open_in_memory()?;
        conn.execute_batch("CREATE TABLE t(a INTEGER); INSERT INTO t VALUES(7)")?;
        let mut stmt=conn.prepare_cached("SELECT * FROM t")?;
        assert_eq!(stmt.column_count(),1);
        { let mut rows=stmt.query([])?; let row=rows.next()?.unwrap();
          assert_eq!(row.get::<_,i64>(0)?,7);
          assert!(matches!(row.get::<_,i64>(1),Err(Error::InvalidColumnIndex(1)))); }
        conn.execute_batch("ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 11")?;
        { let mut rows=stmt.query([])?; let row=rows.next()?.unwrap();
          assert_eq!(row.as_ref().column_count(),2);
          assert_eq!(row.get::<_,i64>(1)?,11);
          assert!(matches!(row.get::<_,i64>(2),Err(Error::InvalidColumnIndex(2)))); }
        drop(stmt);
        conn.execute_batch("ALTER TABLE t DROP COLUMN b")?;
        let mut stmt=conn.prepare_cached("SELECT * FROM t")?;
        { let mut rows=stmt.query([])?; let row=rows.next()?.unwrap();
          assert_eq!(row.as_ref().column_count(),1);
          assert!(matches!(row.get_ref(1),Err(Error::InvalidColumnIndex(1)))); }
        Ok(())
    }
    #[test]
    fn reset_and_conversion_error_do_not_poison_cached_statement() -> Result<()> {
        let conn=Connection::open_in_memory()?;
        conn.execute_batch("CREATE TABLE t(a); INSERT INTO t VALUES(1),('text')")?;
        let mut stmt=conn.prepare_cached("SELECT a FROM t ORDER BY rowid")?;
        { let mut rows=stmt.query([])?;
          assert_eq!(rows.next()?.unwrap().get::<_,i64>(0)?,1);
          assert!(rows.next()?.unwrap().get::<_,i64>(0).is_err()); }
        assert_eq!(stmt.query_row([],|row| row.get::<_,i64>(0))?,1);
        Ok(())
    }
}
'''

def main():
    DATA.mkdir(parents=True,exist_ok=True)
    registry=next((Path(os.environ['CARGO_HOME'])/'registry/src').glob('*/rusqlite-0.40.1'))
    env=os.environ.copy()
    env.update(SQLITE3_STATIC='1',SQLITE3_LIB_DIR=str(ROOT.parent/'sqlite-bench/generated/lib'),
               SQLITE3_INCLUDE_DIR=str(ROOT.parent/'sqlite-bench/generated/sqlite-amalgamation-3530400'))
    env['PATH']=str(WORK/'cargo/bin')+':'+env['PATH']
    metadata={}
    original_lock=tomllib.loads((ROOT/'rust/Cargo.lock').read_text())
    other=lambda lock:[p for p in lock['package'] if p['name']!='rusqlite']
    def upstream_tests(folder,label):
        test_env=env.copy()
        # Separate primary-package test targets avoid accidentally reusing a
        # same-name crate's test artifact when switching source directories.
        test_env['CARGO_TARGET_DIR']=str(WORK/('test-target-'+label))
        result=subprocess.run(['cargo','test','--lib','--manifest-path',str(folder/'Cargo.toml'),
            '--no-default-features','--features','cache,limits,modern_sqlite,hooks'],env=test_env,text=True,capture_output=True)
        output=result.stdout+'\n'+result.stderr
        (DATA/(label+'-tests.txt')).write_text(output)
        failures=sorted(re.findall(r'^---- (.*?) stdout ----$',output,re.M))
        assert result.returncode in [0,101] and 'test result:' in output,output[-3000:]
        return failures,output
    stock_failures,stock_log=upstream_tests(registry,'upstream-stock')
    assert len(stock_failures)==11 and 'no such column:' in stock_log,stock_log[-3000:]
    for kind in ['inline','count','combined']:
        fork=WORK/('rusqlite-'+kind)
        shutil.copytree(registry,fork,dirs_exist_ok=True)
        patch_text=patch(kind,registry)
        diffs=[]
        for name,text in patch_text.items():
            original=(registry/'src'/name).read_text()
            diffs+=list(difflib.unified_diff(original.splitlines(True),text.splitlines(True),
                fromfile='a/src/'+name,tofile='b/src/'+name))
            (fork/'src'/name).write_text(text)
        (DATA/(kind+'.patch')).write_text(''.join(diffs))
        with (fork/'src/raw_statement.rs').open('a') as out: out.write(TESTS)
        project=WORK/('project-'+kind)
        shutil.copytree(ROOT/'rust',project,ignore=shutil.ignore_patterns('target'),dirs_exist_ok=True)
        with (project/'Cargo.toml').open('a') as out:
            out.write('\n[patch.crates-io]\nrusqlite = { path = "'+str(fork)+'" }\n')
        subprocess.run(['cargo','build','--release','--manifest-path',str(project/'Cargo.toml')],env=env,check=True)
        assert other(tomllib.loads((project/'Cargo.lock').read_text()))==other(original_lock),'dependency drift beyond local rusqlite source'
        shutil.copy2(WORK/'target/release/tinystore-metrics-native-bench',WORK/'bin'/('rust-fork-'+kind))
        # Existing upstream unit tests plus schema-reprepare/error regressions.
        failures,test_log=upstream_tests(fork,kind)
        assert failures==stock_failures,('new upstream test failures',kind,failures,stock_failures)
        assert 'schema_reprepare_refreshes_count_and_keeps_invalid_index_errors ... ok' in test_log
        assert 'reset_and_conversion_error_do_not_poison_cached_statement ... ok' in test_log
        metadata[kind]={'crate':'rusqlite 0.40.1','patch_sha256':hashlib.sha256((DATA/(kind+'.patch')).read_bytes()).hexdigest(),
            'original_sha256':{name:hashlib.sha256((registry/'src'/name).read_bytes()).hexdigest() for name in patch_text},
            'patched_sha256':{name:hashlib.sha256((fork/'src'/name).read_bytes()).hexdigest() for name in patch_text},
            'binary_sha256':hashlib.sha256((WORK/'bin'/('rust-fork-'+kind)).read_bytes()).hexdigest(),
            'lock_other_packages_unchanged':True,'upstream_failures_equal_stock':failures,
            'note':'11 upstream tests also fail on stock under the pinned SQLITE_DQS=0 build; all other tests and both added regressions pass'}
        print('Built and tested rusqlite '+kind,flush=True)
    metadata['builder_sha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    (DATA/'build.json').write_text(json.dumps(metadata,indent=2)+'\n')

if __name__=='__main__':main()
