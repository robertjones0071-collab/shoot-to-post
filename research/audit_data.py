"""Read-only historical label audit. Personal rows/reports belong in ignored local storage."""
import argparse,collections,hashlib,json,os,re,sqlite3,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('workspace',type=Path);p.add_argument('output',type=Path);args=p.parse_args();root=args.workspace.resolve();out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
def norm(s):return str(s).replace('\\','/').casefold()
def sha(path):
 with path.open('rb')as f:return hashlib.file_digest(f,'sha256').hexdigest()
def version(r):
 for key in ['rubric_version','rubricVersion']:
  if key in r:return str(r[key])
 match=re.search(r'(?i)rubric\s*(?:version|v)?\s*([0-9]+)',str(r.get('notes','')))
 return match.group(1)if match else'unspecified'
def cats(r):
 c=r.get('categories')or r
 keys=['detail','focus_motion','exposure','noise','color','processing']
 if not isinstance(c,dict)or not all(type(c.get(k))is int for k in keys):return None
 return [c[k]for k in keys]
# The audit snapshot is not a second independent dataset.
journal=root/'aviation-jpeg-processing.jsonl';fingerprint=sha(journal);events=collections.Counter();latest={};parse_errors=0;field_counts=collections.Counter();versions=collections.Counter()
for line in journal.open(encoding='utf-8-sig'):
 if not line.strip():continue
 try:r=json.loads(line)
 except ValueError:parse_errors+=1;continue
 events[str(r.get('outcome','unspecified'))]+=1;field_counts.update(r.keys());versions[version(r)]+=1
 if r.get('original_path'):latest[norm(r['original_path'])]=r
valid=[r for r in latest.values()if type(r.get('score'))in(int,float)and 0<=r['score']<=100 and re.fullmatch('[0-9a-fA-F]{64}',str(r.get('sha256','')))]
by_sha=collections.defaultdict(list)
for r in valid:by_sha[r['sha256'].lower()].append(r)
conflicts={s:rows for s,rows in by_sha.items()if len({r['score']for r in rows})>1}
structured_files=list((root/'output/folder1-20261121').rglob('ratings.json'))+list((root/'tmp/folder1-review').rglob('ratings.json'))+[root/'tmp/folder1-review/batch-001.json']
structured=[];unique_structured={};structured_versions=collections.Counter()
for file in structured_files:
 if not file.is_file():continue
 try:data=json.loads(file.read_text(encoding='utf-8-sig'))
 except ValueError:continue
 rows=data if isinstance(data,list)else data.get('ratings',data.get('rows',[]))
 for row in rows:
  if not isinstance(row,dict)or cats(row)is None:continue
  row=dict(row);row['_file']=str(file.relative_to(root));structured.append(row)
  key=norm(row.get('source_path',''))
  if key:unique_structured[key]=row
structured_unique=list(unique_structured.values());structured_by_sha={};unlinked=0
for r in structured_unique:
 link=latest.get(norm(r.get('source_path','')));s=(link or{}).get('sha256')
 if s:structured_by_sha[s.lower()]=r
 else:unlinked+=1
 structured_versions[version(r)]+=1
# Inventory is read only, and no new indexing is started.
dbpath=root/'aviation-dashboard/data/archive.sqlite';db=sqlite3.connect(dbpath.as_uri()+'?mode=ro',uri=True,timeout=15);db.row_factory=sqlite3.Row;db.execute('PRAGMA query_only=ON')
files=[dict(r)for r in db.execute('SELECT path,size,mtime,sha256,metadata_json,airline,variant,registration,indexing_eligible FROM files WHERE present=1')]
current_by_sha=collections.defaultdict(list);current_by_path={}
for f in files:
 current_by_path[norm(f['path'])]=f
 if f['sha256']:current_by_sha[f['sha256'].lower()].append(f)
labels=[]
for s,rows in by_sha.items():
 if s in conflicts:continue
 rows=sorted(rows,key=lambda r:(r.get('outcome')not in('moved_verified','renamed_copy_created'),not bool(r.get('notes'))))
 r=rows[0];candidates=current_by_sha.get(s,[])
 matched_by='cached exact SHA'
 if not candidates:
  f=current_by_path.get(norm(r.get('new_path','')))
  candidates=[f]if f and not f.get('sha256')else[];matched_by='current path; hash not indexed'
 if not candidates:continue
 f=candidates[0]
 try:metadata=json.loads(f.get('metadata_json')or'{}')
 except ValueError:metadata={}
 stamp=metadata.get('DateTimeOriginal')or metadata.get('CreateDate')or''
 day=stamp[:10]if re.match(r'^\d{4}[:\-]\d{2}[:\-]\d{2}',stamp)else None
 parent=norm(Path(r['original_path']).parent)
 group='day:'+day if day else'original-parent:'+parent
 sr=structured_by_sha.get(s)
 labels.append({'sha256':s,'score':r['score'],'path':f['path'],'bytes':f['size'],'originalPath':r['original_path'],'originalParent':parent,'day':day,'captureStamp':stamp,'labelOutcome':r.get('outcome'),'splitGroup':group,'matchBasis':matched_by,'rubricVersion':version(sr or r),'hasCategoryBreakdown':sr is not None,'categories':cats(sr)if sr else None,'motion':(sr or{}).get('motion_mode'),'confidence':(sr or{}).get('confidence'),'notes':r.get('notes',''),'phash':None,'dhash':None})
features={r['sha256']:(r['phash'],r['dhash'])for r in db.execute('SELECT sha256,phash,dhash FROM features')}
for r in labels:
 if r['sha256']in features:r['phash'],r['dhash']=features[r['sha256']]
assessments={r[0]:r[1]for r in db.execute('SELECT engine,count(*)FROM assessments GROUP BY engine')};decisions=dict(db.execute('SELECT count(*)total,sum(rating IS NOT NULL)numericRatings FROM decisions').fetchone());db.close()
limits=[30,25,15,10,10,10]
valid_categories=[r for r in structured_unique if all(0<=v<=m for v,m in zip(cats(r),limits))]
notes=collections.Counter(str(r.get('notes','')).strip()for r in latest.values()if r.get('notes'))
report={'journalSha256':fingerprint,'journalEvents':dict(events),'totalJournalEvents':sum(events.values()),'parseErrors':parse_errors,'uniqueOriginalPaths':len(latest),'scoredLatestRowsWithSha':len(valid),'uniqueScoredSha':len(by_sha),'conflictingScoreSha':len(conflicts),'journalExplicitVersionEvents':dict(versions),'currentPresentFiles':len(files),'currentFilesWithIndexedSha':sum(bool(f['sha256'])for f in files),'labelledCurrentUniqueSha':len(labels),'exactCachedHashLinks':sum(r['matchBasis']=='cached exact SHA'for r in labels),'pathOnlyLinksNeedVerification':sum(r['matchBasis']!='cached exact SHA'for r in labels),'scoreDistribution':dict(sorted(collections.Counter(r['score']for r in labels).items())),'metadataDayAvailable':sum(bool(r['day'])for r in labels),'candidateSplitGroups':len({r['splitGroup']for r in labels}),'verifiedLatestScoredRows':sum(r.get('outcome')in('moved_verified','renamed_verified','unchanged_verified','renamed_copy_created')for r in valid),'structuredCurrentLinkedSha':sum(r['hasCategoryBreakdown']for r in labels),'structuredCurrentCaptureDays':len({r['day']for r in labels if r['hasCategoryBreakdown']}),'structuredCurrentSumMatchesLabel':sum(sum(r['categories'])==r['score']for r in labels if r['hasCategoryBreakdown']),'structuredFiles':len(structured_files),'structuredRawRows':len(structured),'structuredUniqueOriginalPaths':len(structured_unique),'structuredLinkedSha':len(structured_by_sha),'structuredUnlinked':unlinked,'structuredCategoryBoundsValid':len(valid_categories),'structuredCategorySumMatchesRecordedScore':sum(sum(cats(r))==r.get('score')for r in valid_categories),'structuredRubricVersions':dict(structured_versions),'structuredCompletenessPresent':sum(any(k in r for k in ['aircraft_completeness','aircraftCompleteness','completeness'])for r in structured_unique),'structuredZonesPresent':sum(bool(r.get('zones'))for r in structured_unique),'structuredMotionPresent':sum(bool(r.get('motion_mode'))for r in structured_unique),'structuredConfidencePresent':sum(bool(r.get('confidence'))for r in structured_unique),'structuredReviewerIdentityPresent':sum(any(k in r for k in ['reviewer','reviewed_by'])for r in structured_unique),'structuredReviewTimestampPresent':sum(any(k in r for k in ['reviewedAt','reviewed_at'])for r in structured_unique),'notesNoCropDeduction':sum('no composition or crop deductions'in str(r.get('notes','')).lower()for r in structured_unique),'distinctJournalNotes':len(notes),'largestRepeatedNoteCount':max(notes.values(),default=0),'archiveAssessmentEngines':assessments,'savedHumanDecisions':decisions,'snapshotExcludedFromIndependentExamples':True,'batchPlanFiles':len(list(root.glob('aviation-*-batch-*.json'))),'journalUnchanged':sha(journal)==fingerprint,'limits':['Historical scores are recorded labels, not independently adjudicated ground truth.','Availability/hash linkage audit uses existing inventory; only pilot sample bytes will be reverified.','Structured copies are deduplicated by original path, not assumed independent examples.']}
(out/'audit.json').write_text(json.dumps(report,indent=2));(out/'labels.private.json').write_text(json.dumps(labels));(out/'structured.private.json').write_text(json.dumps(structured_unique));(out/'conflicts.private.json').write_text(json.dumps(conflicts));print(json.dumps(report),flush=True)
