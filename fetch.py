from typing import List, Tuple
from selenium import webdriver
import os
import time
from bs4 import BeautifulSoup
from dotenv import load_dotenv

# ```
# COBALT_SESSION=key
# ```
load_dotenv()
cobalt_session = os.getenv('COBALT_SESSION')
pages = 130

if cobalt_session is None:
  print(f'Error: missing COBALT_SESSION property in {os.getcwd()}/.env file')
  exit(1)

def make_driver():
	options = webdriver.ChromeOptions()
	options.add_argument("--enable-javascript")
	return webdriver.Chrome(options=options)

print("Fetching unfetched pages")
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
		else:
			print(f'Encountered automation blocked response while fetching creature listing page {page_idx+1}')
			break

		time.sleep(1)

print("Scanning fetched pages")
unfetched_monsters = []
for page_idx in range(0, pages):
	page_file = f"pages/{page_idx+1}.html"
	if not os.path.exists(page_file):
		continue
	print(f"  {page_file}")
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
		if not os.path.exists(f".{link_path}.html"):
			unfetched_monsters.append(link_path)

print(f"Found {len(unfetched_monsters)} creatures which have not been downloaded. Adding urls to 'monsters.txt'.")
with open('monsters.txt', 'wt+', encoding="utf-8") as file:
	for path in unfetched_monsters:
		file.write(f"https://www.dndbeyond.com{path}\n")
