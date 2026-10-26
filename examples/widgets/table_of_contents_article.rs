//! Original English factual summary informed by Wikipedia's “Solar System”.
//! No Wikipedia prose or media is reproduced. See README's example attribution.

use rutter::HeadingLevel;

pub(super) const ATTRIBUTION: &str = "Source: Solar System — Wikipedia contributors. Original English factual summary, shortened and reorganized for this demo; no article prose copied. https://en.wikipedia.org/wiki/Solar_System (source text: CC BY-SA 4.0).";

pub(super) struct Section {
    pub level: HeadingLevel,
    pub title: &'static str,
    pub paragraphs: &'static [&'static str],
}

pub(super) const SECTIONS: &[Section] = &[
    Section {
        level: HeadingLevel::H1,
        title: "The Solar System",
        paragraphs: &[
            "Earth belongs to a family of worlds held together by the Sun's gravity. Eight planets travel around this star, accompanied by moons, dwarf planets, asteroids, comets and dust. These objects differ enormously in size and composition. Together they preserve clues to the conditions in which our planetary system began, about 4.6 billion years ago.",
            "A journey outward crosses several distinct regions rather than an evenly spaced collection of planets. Small rocky worlds occupy the inner region. Giant planets lie farther away, followed by populations of icy bodies. Most of the space between these objects is almost empty, so familiar diagrams usually enlarge the worlds or compress their distances to make them visible.",
        ],
    },
    Section {
        level: HeadingLevel::H2,
        title: "Origins and motion",
        paragraphs: &[
            "The system developed when part of a cloud of gas and dust collapsed under gravity. Material gathered in the centre to form the young Sun, while a rotating disc surrounded it. Within that disc, solid particles joined into larger bodies. Repeated encounters and collisions helped build planets, but also left smaller remnants that never became part of a large world.",
        ],
    },
    Section {
        level: HeadingLevel::H3,
        title: "Orbits and distances",
        paragraphs: &[
            "Most planets orbit in roughly the same plane and in the same direction, a legacy of the rotating disc. Their paths are ellipses rather than perfect circles. Farther planets take longer to complete a circuit. Astronomers often describe distances in astronomical units: one unit is approximately the average distance between Earth and the Sun, about 150 million kilometres.",
        ],
    },
    Section {
        level: HeadingLevel::H2,
        title: "The Sun",
        paragraphs: &[
            "The Sun contains almost all the mass in the Solar System, making it the dominant influence on planetary motion. Deep inside it, nuclear fusion converts hydrogen into helium and releases energy. That energy eventually escapes as radiation. Sunlight warms planetary surfaces and atmospheres, although distance, clouds and atmospheric composition give each world a very different response to that incoming energy.",
        ],
    },
    Section {
        level: HeadingLevel::H3,
        title: "The solar wind",
        paragraphs: &[
            "The Sun also sends a stream of charged particles into space. This solar wind creates a vast surrounding region called the heliosphere. Its outer boundary marks a change in the particle environment, not the end of the Sun's gravitational influence. The distinction matters when describing spacecraft that have entered interstellar space but remain much closer than the most distant bodies associated with the Sun.",
        ],
    },
    Section {
        level: HeadingLevel::H2,
        title: "The inner worlds",
        paragraphs: &[
            "Mercury, Venus, Earth and Mars are the terrestrial planets. Rock and metal make up much of their bulk, and all have solid surfaces. Mercury is the smallest planet and the closest to the Sun. Venus has a dense atmosphere and an extremely hot surface. Earth has extensive liquid-water oceans, while Mars displays a cold landscape with evidence of a wetter past.",
        ],
    },
    Section {
        level: HeadingLevel::H3,
        title: "The asteroid belt",
        paragraphs: &[
            "Between Mars and Jupiter, many small rocky bodies occupy the asteroid belt. It is a broad region, not a tightly packed obstacle course: typical separations are large compared with the objects themselves. Ceres, the largest member, is classified as a dwarf planet. The belt's varied objects help researchers investigate materials and events from the early history of the planetary system.",
        ],
    },
    Section {
        level: HeadingLevel::H2,
        title: "The giant planets",
        paragraphs: &[
            "Jupiter and Saturn are gas giants dominated by hydrogen and helium. Jupiter is the largest planet, while Saturn is especially familiar for its bright rings. Uranus and Neptune are called ice giants because their interiors contain a larger proportion of heavier substances. These names describe broad differences in composition; they do not mean that the outer planets are simple balls of gas or exposed frozen water.",
        ],
    },
    Section {
        level: HeadingLevel::H3,
        title: "Moons and rings",
        paragraphs: &[
            "All four giant planets have rings and many natural satellites. Their moons are worlds in their own right, with diverse surfaces and histories. Some show strong geological activity; others preserve heavily cratered terrain. Rings consist of numerous separate particles orbiting the planet rather than a single solid disc. Studying these smaller systems reveals how gravity shapes material on scales far below that of a planetary orbit.",
        ],
    },
    Section {
        level: HeadingLevel::H2,
        title: "Beyond Neptune",
        paragraphs: &[
            "The planetary region does not contain everything that circles the Sun. Beyond Neptune lies the Kuiper belt, home to Pluto and many other icy objects. Pluto is a dwarf planet, not one of the eight planets. More distant populations follow a wide range of elongated and tilted orbits, reminding us that the outer system is more complex than a set of neat concentric rings.",
        ],
    },
    Section {
        level: HeadingLevel::H3,
        title: "Comets and distant frontiers",
        paragraphs: &[
            "When an icy comet approaches the Sun, warming can release gas and dust, producing a surrounding coma and sometimes long tails. Some comets return on relatively short schedules; others follow paths that extend far beyond the planets. A much more distant reservoir, the proposed Oort cloud, helps explain the arrival of long-period comets, although its individual members are too remote to observe directly as a population.",
            "Exploration combines telescope observations with spacecraft measurements. Flybys reveal brief close views, orbiters follow changes over time, and landers examine particular surfaces. Each method answers different questions. Even after visits to every major planet, the distant Solar System remains difficult to survey, so a map of our celestial neighbourhood is also a record of what we have yet to learn.",
        ],
    },
];
