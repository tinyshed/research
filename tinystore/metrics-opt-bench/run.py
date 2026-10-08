"""Shared workload definitions from the preceding full metrics experiment."""

EPOCH=1700000000000

NORMAL_NOW=EPOCH+6000

WRITE_CASES={
 'ingest_append1':'head','ingest_replace1':'head','ingest_scrape100':'scrape',
 'ingest_register8':'empty','ingest_shuffled240':'head','ingest_duplicates240':'head',
 'maintain_ready':'ready','expire_all':'sealed',
}

READ_CASES=['read_head_point','read_head_full8','read_sealed_point','read_sealed_boundary','read_sealed_full8','read_sealed_full16','read_filtered_one','stream_sealed_full8','read_where','read_prefix','read_noneof','read_since']

AGGREGATE_CASES=['aggregate_'+op for op in ['count','sum','avg','min','max','delta','increase','rate','cut_sum','cut_avg','grouped_sum','grouped_increase']]

CASES=list(WRITE_CASES)+READ_CASES+AGGREGATE_CASES

EDGE_CASES=['read_edge']+['aggregate_edge_'+op+'_'+str(i) for op,i in [('sum',0),('sum',1),('sum',3),('sum',4),('sum',5),('sum',6),('sum',8),('sum',9),('sum',11),('avg',6),('avg',8),('avg',9),('min',5),('max',5),('increase',10),('rate',10)]]

def fixture_for(case):
 return WRITE_CASES.get(case,'head' if case.startswith('read_head') else 'edge' if case in EDGE_CASES else 'sealed')
