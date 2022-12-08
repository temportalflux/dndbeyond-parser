from typing import List, Tuple
from selenium import webdriver
import os
import time
from bs4 import BeautifulSoup
from pathlib import Path

pages = 130
cobalt_session = "eyJhbGciOiJkaXIiLCJlbmMiOiJBMTI4Q0JDLUhTMjU2In0..GZ_zgS_EZ8frh0SC2e7_PA.IotUw1dt4RH-XmKcZnXXD4LT0666qgLxV9to1gRG58Jyomoz_UIrIbh19CDxZHHO.jdVq76aAM6pcj4RYYsk_hg"
cobalt_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJodHRwOi8vc2NoZW1hcy54bWxzb2FwLm9yZy93cy8yMDA1LzA1L2lkZW50aXR5L2NsYWltcy9uYW1laWRlbnRpZmllciI6IjEwMDAxNDc5NyIsImh0dHA6Ly9zY2hlbWFzLnhtbHNvYXAub3JnL3dzLzIwMDUvMDUvaWRlbnRpdHkvY2xhaW1zL25hbWUiOiJ0ZW1wb3J0YWxmbHV4IiwiaHR0cDovL3NjaGVtYXMueG1sc29hcC5vcmcvd3MvMjAwNS8wNS9pZGVudGl0eS9jbGFpbXMvZW1haWxhZGRyZXNzIjoiZHVzdGluLnlvc3QudEBnbWFpbC5jb20iLCJkaXNwbGF5TmFtZSI6IlRlbXBvcnRhbEZsdXgiLCJodHRwOi8vc2NoZW1hcy5taWNyb3NvZnQuY29tL3dzLzIwMDgvMDYvaWRlbnRpdHkvY2xhaW1zL3JvbGUiOlsiUmVnaXN0ZXJlZCBVc2VycyIsIkNyaXRpY2FsIFJvbGUgRWxlY3Rpb24gMjAxOSAtIFZvdGVkIl0sImh0dHA6Ly9zY2hlbWFzLmRuZGJleW9uZC5jb20vd3MvMjAxOS8wOC9pZGVudGl0eS9jbGFpbXMvc3Vic2NyaWJlciI6IlRydWUiLCJodHRwOi8vc2NoZW1hcy5kbmRiZXlvbmQuY29tL3dzLzIwMTkvMDgvaWRlbnRpdHkvY2xhaW1zL3N1YnNjcmlwdGlvbnRpZXIiOiJNYXN0ZXIiLCJuYmYiOjE2NzA1MTQzMzksImV4cCI6MTY3MDUxNDYzOSwiaXNzIjoiZG5kYmV5b25kLmNvbSIsImF1ZCI6ImRuZGJleW9uZC5jb20ifQ.8vFhR3julIZnuD2s91q03jGQJmFhCfmcqCNZaAGSlbw"

def make_driver():
	options = webdriver.ChromeOptions()
	options.add_argument("--enable-javascript")
	return webdriver.Chrome(options=options)

print("Gathering list of unfetched pages")
if not os.path.exists("pages"):
	os.makedirs("pages")
unfetched_pages: List[Tuple[int, str]] = list()
for page_idx in range(0, pages):
	page_file = f"pages/{page_idx+1}.html"
	if not os.path.exists(page_file):
		unfetched_pages.append((page_idx, page_file))
if len(unfetched_pages) > 0:
	print("Fetching missing pages")
	driver = make_driver()
	for (page_idx, page_file) in unfetched_pages:
		url = f"https://www.dndbeyond.com/monsters?page={page_idx+1}&sort=cr"
		print(f"Fetching page {page_idx+1}")

		driver.get(url)
		driver.add_cookie({
			'name': 'CobaltSession',
			'domain': 'dndbeyond.com',
			'value': cobalt_session,
		})
		html = str(driver.page_source)
		if not html.__contains__("Access to this page has been denied."):
			with open(page_file, "wt+", encoding="utf-8") as f:
				f.write(html)

		time.sleep(1)

print("Reading fetched pages")
source_page_files: List[str] = list()
for root, dirs, files in os.walk("pages"):
	for name in files:
		source_page_files.append(os.path.join(root, name))
unfetched_monsters: List[str] = []
for page_file in source_page_files:
	with open(page_file, "r", encoding="utf-8") as file:
		listings_page = BeautifulSoup(file.read(), "html.parser")

	body = listings_page.find("body")
	site = body.find(id="site")
	main = site.find(id="site-main")
	container = main.find(class_="container")
	content = container.find(id="content")
	primary_content = content.find(class_="primary-content")

	list_container = primary_content.find(class_="listing-container")
	list_body = list_container.find(class_="listing-body")
	list = list_container.find("ul", class_="listing")
	list_items = list.find_all(class_="info")
	for item in list_items:
		title_block = item.find(class_="monster-name")
		name_span = title_block.find("span", class_="name")
		name_link = name_span.find("a", class_="link")
		link_path = name_link.get("href")
		src_path = f".{link_path}.html"
		if not os.path.exists(src_path):
			unfetched_monsters.append((src_path, link_path))

with open('monsters.txt', 'wt+', encoding="utf-8") as file:
	for (_, path) in unfetched_monsters:
		file.write(f"https://www.dndbeyond.com{path}\n")
