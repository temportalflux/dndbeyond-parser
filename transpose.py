from typing import Dict, List, Tuple
import os
import time
from bs4 import BeautifulSoup, Tag
from cuddle import Document, Node, NodeList
import re

class CreatureParser:

  info: Tag

  def __init__(self, html):
    soup = BeautifulSoup(html, "html.parser")
    content = soup.find('body').find(id='site').find(id='site-main').find(class_='container').find(id='content')
    self.info = content.find(class_='primary-content').find(class_='monster-details').find(class_='more-info')
    
  def image_url(self) -> str:
    aside = self.info.find(class_='details-aside')
    link = aside.find(class_='image').find('a')
    return link.get("href")
  
  def parse(self) -> 'Creature':
    creature = Creature()
    creature.image_url = self.image_url()

    creature.environment_tags = list()
    tags = self.info.find('footer').find('p', class_='tags')
    for span in tags.find_all('span', class_='environment-tag'):
      creature.environment_tags.append(str(span.string))

    content = self.info.find(class_='detail-content')
    if 'Stat Block':
      stat_block = content.find(class_='mon-stat-block')
      if 'Header':
        header = stat_block.find(class_='mon-stat-block__header')
        name_link = header.find(class_='mon-stat-block__name').find('a')
        creature.name = str(name_link.string).strip()
        creature.url = name_link.get('href')
      if 'Meta':
        meta = stat_block.find(class_='mon-stat-block__meta')
        matched = re.match(r'(?P<size>[a-zA-Z]*) (?P<kind>[a-zA-Z]*), (?P<alignment>.*)', str(meta.string).strip())
        creature.size = matched.group('size')
        creature.kind = matched.group('kind')
        creature.alignment = matched.group('alignment')
      if 'Attributes':
        attribs = stat_block.find(class_="mon-stat-block__attributes")
        for attribute in attribs.find_all(class_="mon-stat-block__attribute"):
          attr: Tag = attribute
          label = attr.find(class_="mon-stat-block__attribute-label")
          
          value_set = attr.find(class_="mon-stat-block__attribute-data")
          if value_set is None:
            value_set = attr.find(class_="mon-stat-block__attribute-value")

          value = value_set.find(class_="mon-stat-block__attribute-data-value")
          extra = value_set.find(class_="mon-stat-block__attribute-data-extra")
          label = label.string.strip()
          value = value.string.strip()
          if extra is not None:
            extra = extra.string.strip().removeprefix('(').removesuffix(')')
          
          if label == 'Hit Points':
            creature.hit_points = (int(value), extra)
          elif label == 'Armor Class':
            creature.armor_class = (int(value), extra)
          elif label == 'Speed':
            re_speed = re.compile(r'(?P<mode>[a-zA-Z]*)? ?(?P<amount>[0-9]*) ft.')
            creature.speed = []
            for item in value.split(','):
              matched = re_speed.match(item.strip())
              mode = matched.group('mode')
              if mode == '':
                mode = None
              creature.speed.append((int(matched.group('amount')), mode))
          else:
            print(f"[WARN] Missing support for attribute: {label} {value} {extra}")
      if 'Stats':
        ability_block = stat_block.find(class_='mon-stat-block__stat-block').find(class_='ability-block')
        stats = ['str', 'dex', 'con', 'int', 'wis', 'cha']
        creature.ability_scores = {}
        for stat in stats:
          item = ability_block.find(class_=f'ability-block__stat--{stat}')
          item = item.find(class_='ability-block__data').find(class_='ability-block__score')
          creature.ability_scores[stat] = int(item.string)
      if 'Tidbits':
        # > .mon-stat-block__tidbits
        #   > .mon-stat-block__tidbit (list)
        #     > .mon-stat-block__tidbit-label > text = [saving throws / sense/ skills / languages]
        #     > .mon-stat-block__tidbit-data > text (stripped) = [value]
        #   > .mon-stat-block__tidbit-container > .mon-stat-block__tidbit (list)
        #     > .mon-stat-block__tidbit-label > text = [cr / profbonus]
        #     > .mon-stat-block__tidbit-data > text (stripped) = [value]
        tidbits = stat_block.find(class_='mon-stat-block__tidbits')
        container = tidbits.find(class_='mon-stat-block__tidbit-container')
        all_tidbits = tidbits.find_all(class_='mon-stat-block__tidbit') + container.find_all(class_='mon-stat-block__tidbit')
        re_skill = re.compile(r'(?P<name>[a-zA-Z]*) \+(?P<amount>[0-9]*)')
        re_prefixed_dist = re.compile(r'(?P<name>[a-zA-Z]*) (?P<amount>[0-9]*) ft.')
        for tidbit in all_tidbits:
          tidbit: Tag = tidbit
          label = tidbit.find(class_='mon-stat-block__tidbit-label').string
          data = tidbit.find(class_='mon-stat-block__tidbit-data').get_text().strip()
          if label == "Saving Throws":
            creature.saving_throws = {}
            for skill_bonus in data.split(','):
              matched = re_skill.match(skill_bonus.strip())
              creature.saving_throws[matched.group('name').lower()] = int(matched.group('amount'))
          elif label == "Skills":
            creature.skills = {}
            for skill_bonus in data.split(','):
              matched = re_skill.match(skill_bonus.strip())
              creature.skills[matched.group('name')] = int(matched.group('amount'))
          elif label == "Senses":
            creature.senses = {}
            for sense in data.split(','):
              sense = sense.strip()
              if sense.startswith("Passive Perception"):
                sense = int(sense.removeprefix("Passive Perception").strip())
                creature.senses["Passive Perception"] = (sense, None)
              else:
                matched = re_prefixed_dist.match(sense.strip())
                creature.senses[matched.group('name')] = (int(matched.group('amount')), 'ft')
          elif label == "Languages":
            creature.languages = {}
            for lang_dist in data.split(','):
              matched = re_prefixed_dist.match(lang_dist.strip())
              if matched is not None:
                creature.languages[matched.group('name')] = int(matched.group('amount'))
              else:
                creature.languages[lang_dist.strip()] = None
          elif label == "Challenge":
            matched = re.match(r'(?P<cr>[0-9]*) \((?P<xp>[0-9,]*) XP\)', data.strip())
            creature.challenge_rating = (int(matched.group('cr')), int(matched.group('xp').replace(',', '')))
          elif label == "Proficiency Bonus":
            creature.proficiency_bonus = int(data.strip().removeprefix("+"))
          else:
            print(f"[WARN] Missing support for tidbit named '{label}', value = '{data}'")
      if 'Description Blocks':
        # > .mon-stat-block__description-blocks > .mon-stat-block__description-block (list)
        #   > .mon-stat-block__description-block-heading > text = [section]
        #   > .mon-stat-block__description-block-content > p (list)
        #     > em > strong > text = [property label]
        #     > text OR OTHER PARSER = [property description]
        #     this block is especially context dependent, because there is text at all layers
        pass


    lore_content = content.find(class_='more-info-content')

    return creature

class Creature:
  name: str
  url: str
  image_url: str
  size: str
  kind: str
  alignment: str
  hit_points: Tuple[int, str]
  armor_class: Tuple[int, str]
  speed: List[Tuple[int, str | None]]
  ability_scores: Dict[str, int]
  saving_throws: Dict[str, int]
  skills: Dict[str, int]
  senses: Dict[str, Tuple[int, str | None]]
  languages: Dict[str, int | None]
  challenge_rating: Tuple[int, int]
  proficiency_bonus: int
  environment_tags: List[str]

  def to_kdl(self) -> Node:
    children = [
      Node("name", None, arguments=[self.name]),
      Node("url", "url", arguments=[self.url]),
      Node("image", "url", arguments=[self.image_url]),
      Node("size", None, arguments=[self.size]),
      Node("kind", None, arguments=[self.kind]),
      Node("alignment", None, arguments=[self.alignment]),
      Node("hit_points", None, arguments=[self.hit_points[0], self.hit_points[1]]),
      Node("armor_class", None, arguments=[self.armor_class[0], self.armor_class[1]]),
      self.speed_kdl(),
      Node("ability_scores", None, properties=self.ability_scores),
      Node("saving_throws", None, properties=self.saving_throws),
      Node("skills", None, properties=self.skills),
      Node("senses", None, children=self.senses_kdl()),
      Node("languages", None, children=self.languages_kdl()),
      Node("challenge_rating", None, arguments=[self.challenge_rating[0]], properties={'xp': self.challenge_rating[1]}),
      Node("proficiency_bonus", None, arguments=[self.proficiency_bonus]),
      Node("environments", None, arguments=self.environment_tags),
    ]
    return Node("creature", "Creature", children=children)
  
  def speed_kdl(self) -> Node:
    regular_speed: int
    alternate_speeds: Dict[str, int] = {}
    for (amt, mode) in self.speed:
      if mode is None:
        regular_speed = amt
      else:
        alternate_speeds[mode] = amt
    return Node("speed", None, arguments=[regular_speed], properties=alternate_speeds)

  def senses_kdl(self) -> List[Node]:
    nodes = []
    for (sense, (amount, unit)) in self.senses.items():
      if unit is None:
        nodes.append(Node(sense, None, arguments=[amount]))
      else:
        nodes.append(Node(sense, None, properties={'range': amount, 'unit': unit}))
    return nodes

  def languages_kdl(self) -> List[Node]:
   return [Node(name, None, properties=({'range': dist, 'unit': 'ft'} if dist is not None else {})) for (name, dist) in self.languages.items()]

import cuddle
from pathlib import Path

path = Path('monsters/16762-aboleth.html')
with open(path, 'r', encoding="utf-8") as file:
  html = file.read()
parser = CreatureParser(html)
creature = parser.parse()
print(cuddle.dumps(cuddle.NodeList([creature.to_kdl()])))
