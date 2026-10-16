#!/usr/bin/env python3
"""Small original adversarial fixture; diagnostics, not population quality."""
import json
from pathlib import Path
docs=[
 ('Кошки','Кошки спят на диване.'),('Собаки','Собаки гуляют по улице.'),
 ('EN tests','Developers are running tests.'),('Cats','The cats sleep on a sofa.'),
 ('ZH database','我喜欢数据库系统。'),('ZH books','我买了一本数据库设计的书。'),
 ('ZH university','北京大学的研究生正在学习。'),('ZH students','北京的大学生喜欢运动。'),
 ('Mixed','SQLite хранит базы данных. Developers are running tests. 我喜欢数据库系统。'),
 ('Репликация','Как работает репликация SQLite. Разбираемся в WAL и распределённых системах.'),
 ('RU metal','Сталь используется для изготовления инструментов.'),('RU verb','Дети стали читать книги.'),
 ('Люди','Люди работают в лаборатории.'),('Лес','Ёлки растут в лесу.'),
 ('DE houses','Die Häuser stehen am Fluss.'),('FR horses','Les chevaux courent dans le pré.'),
 ('JA database','データベースシステムを学ぶ。'),('TH','ฉันชอบระบบฐานข้อมูล'),
 ('Unicode width','ＡＢＣ Café Straße İstanbul'),('EN unrelated','The building has a roof.')]
queries=[
 ('ru','кошка',[1]),('ru','собака',[2]),('en','run test',[3,9]),('en','cat',[4]),
 ('zh','数据库',[5,6,9]),('zh','北京大学',[7]),('mixed','SQLite 数据库',[5,6,9,10]),
 ('ru-semantic','реплицировать базу',[9,10]),('ru-stem-collision','сталь',[11]),
 ('ru-suppletion','человек',[13]),('ru-yo','елка',[14]),('de','Haus',[15]),
 ('fr-irregular','cheval',[16]),('ja','データベース',[17]),('th','ฐานข้อมูล',[18]),
 ('unicode','abc',[19]),('unicode-casefold','STRASSE',[19]),('unicode-casefold','istanbul',[19])]
root=Path(__file__).resolve().parent/'corpus';root.mkdir(exist_ok=True)
rows=[{'id':i+1,'key':str(i+1),'title':title,'body':body}for i,(title,body)in enumerate(docs)]
qs=[{'key':str(i),'language':lang,'text':q,'relevant':rel,'judged':{str(d['id']):int(d['id']in rel)for d in rows}}for i,(lang,q,rel)in enumerate(queries)]
(root/'gold.json').write_text(json.dumps({'docs':rows,'queries':qs},ensure_ascii=False))
