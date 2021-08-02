from bs4 import BeautifulSoup

with open(r'cache/', 'rt') as f:
    content = f.read()
    soup = BeautifulSoup(content, 'lxml')
