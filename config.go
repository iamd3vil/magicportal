package main

import (
	"fmt"
	"log"
	"strings"

	"github.com/knadh/koanf"
	"github.com/knadh/koanf/parsers/json"
	"github.com/knadh/koanf/parsers/toml"
	"github.com/knadh/koanf/providers/file"
)

type Config struct {
	Mode            string              `koanf:"mode"`
	MaxPacketSize   int                 `koanf:"max_packet_size"`
	MulticastGroups []CfgMulticastGroup `koanf:"multicast_groups"`
	CfgNats         CfgNats             `koanf:"nats"`
	CfgAgent        CfgAgent            `koanf:"agent"`
}

type CfgNats struct {
	NatsURL     []string `koanf:"nats_url"`
	AuthEnabled bool     `koanf:"auth_enabled"`
	Username    string   `koanf:"username"`
	Password    string   `koanf:"password"`
	TLS         bool     `koanf:"tls"`
}

type CfgAgent struct {
	SendAsUnicast bool              `koanf:"send_as_unicast"`
	UnicastAddrs  map[string]string `koanf:"unicast_addrs"`
}

type CfgMulticastGroup struct {
	MulticastAddr string `koanf:"multicast_addr"`
	Interface     string `koanf:"interface"`
}

func initConfig(cfgPath string) (Config, error) {
	c := Config{}
	ko := koanf.New(".")

	var parser koanf.Parser
	ext, err := getFileExtension(cfgPath)
	if err != nil {
		return c, err
	}

	switch ext {
	case "json":
		parser = json.Parser()
	case "toml":
		parser = toml.Parser()
	}

	if err := ko.Load(file.Provider(cfgPath), parser); err != nil {
		return Config{}, fmt.Errorf("error loading config: %v", err)
	}

	if err := ko.Unmarshal("", &c); err != nil {
		return c, fmt.Errorf("error unmarshalling config: %v", err)
	}

	log.Printf("nats url: %v", ko.StringMap("agent.unicast_addrs"))

	return c, nil
}

func getFileExtension(fp string) (string, error) {
	list := strings.Split(fp, ".")
	if len(list) < 2 {
		return "", fmt.Errorf("couldn't recognize file extension of config")
	}

	return list[1], nil
}
