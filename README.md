# D&D Scrape
==========

## Usage

1. Login to [D&D Beyond](dndbeyond.com), and find the cookie named `CobaltSession`. In chrome, you can find this cookie in the Dev Tools. In the `Application` tab, under the `Cookies` section in the sidebar, you should find the `https://www.dndbeyond.com` group. In this group, once you are logged-in, there should be a cookie named `CobaltSession`.
2. In the directory you will run the scraper from, save the below contents to a file named `cookies.txt`. Replace the characters `COOKIE` with the value of the `CobaltSession` cookie in the Dev Tools.
```
CobaltSession=COOKIE
```
3. Run `dndscrape` in the directory with the `cookies.txt` file.