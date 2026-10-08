/* Research-only readback utility. SQLite is the external pinned archive. */
#include <stdio.h>
#include <stdlib.h>
#include "sqlite3.h"
static void check(int rc,sqlite3 *db){if(rc!=SQLITE_OK){fprintf(stderr,"SQLite readback failed: %s\n",sqlite3_errmsg(db));exit(1);}}
static void query(sqlite3 *db,const char *label,const char *sql){sqlite3_stmt *s=0;check(sqlite3_prepare_v2(db,sql,-1,&s,0),db);int rc;while((rc=sqlite3_step(s))==SQLITE_ROW){const unsigned char *v=sqlite3_column_text(s,0);printf("%s\t%s\n",label,v?(const char*)v:"");}if(rc!=SQLITE_DONE){fprintf(stderr,"step failed\n");exit(1);}check(sqlite3_finalize(s),db);}
int main(int argc,char **argv){if(argc!=3)return 2;sqlite3 *db=0;check(sqlite3_open_v2(argv[1],&db,SQLITE_OPEN_READWRITE,0),db);check(sqlite3_exec(db,"pragma foreign_keys=1;pragma busy_timeout=5000;pragma synchronous=FULL;pragma fullfsync=1;pragma checkpoint_fullfsync=1;pragma cache_size=-1024",0,0,0),db);if(argv[2][0]=='w'){check(sqlite3_exec(db,"pragma cache_size=-4096;pragma journal_mode=WAL",0,0,0),db);}else{check(sqlite3_exec(db,"pragma query_only=1",0,0,0),db);}
query(db,"version","select sqlite_version()");query(db,"source_id","select sqlite_source_id()");query(db,"compile_option","pragma compile_options");
const char *pragmas[]={"page_size","synchronous","cache_size","wal_autocheckpoint","fullfsync","checkpoint_fullfsync","foreign_keys","busy_timeout","query_only","journal_mode"};for(size_t i=0;i<sizeof(pragmas)/sizeof(*pragmas);i++){char sql[128];snprintf(sql,sizeof(sql),"pragma %s",pragmas[i]);query(db,pragmas[i],sql);}check(sqlite3_close(db),db);return 0;}
