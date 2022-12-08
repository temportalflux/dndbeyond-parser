# DnDBeyond Monster List Scraper
==========

Fetches the list of creatures from dndbeyond.com/monsters, assuming the total number of pages is known.

## Setup

1. Login to [D&D Beyond](dndbeyond.com), and find the cookie named `CobaltSession`. In chrome, you can find this cookie in the Dev Tools. In the `Application` tab, under the `Cookies` section in the sidebar, you should find the `https://www.dndbeyond.com` group. In this group, once you are logged-in, there should be a cookie named `CobaltSession`.
2. In the directory you will run the scraper from, save the below contents to a file named `.env`. Replace the characters `session` with the value of the `CobaltSession` cookie in the Dev Tools.
```
COBALT_SESSION=session
```

## Usage: Fetch

Run the `fetch.py` script. The script will look for a `pages` and `monsters` directories to compare fetched data against. If there are any missing pages, it will attempt to fetch them from dndbeyond.

After all pages have been fetched, it will determine which creature html files have not been downloaded, based on the listings available in the downloaded page html files. The list of missing monster html urls is collected in a `monsters.txt` file.

Due to automation blockers on each monster page, users will need to download the pages manually. You can do so by using the [Copy All Urls](https://chrome.google.com/webstore/detail/copy-all-urls/djdmadneanknadilpjiknlnanaolmbfk) and [SingleFile](https://chrome.google.com/webstore/detail/singlefile/mpiodijhokgodhhofbcjdecpffjipkle) chrome extensions. The former will allow you to open multiple urls at once, and the latter has an option to `Save All Tabs`. By opening 20 monster pages at once for download, and configuring `SingleFile` to use the output file name `{url-last-segment}.html`, the monster files can be downloaded with little mental power (though it does take a handful of hours). Every 100 pages (5 batches of 20) or so, dndbeyond will detect automation at play, but this can be by-passed manually.

When you need to refresh what has been fetched (e.g. new books have been added) you may need to delete the downloaded page listings to re-fetch them or (e.g. you buy new monsters/books) you may need to go back and manually download new listings.

## Usage: Transpose

The transpose script will analyze each creature's html file and extract relevant data to be exported as [`kdl`](https://kdl.dev/) files.
