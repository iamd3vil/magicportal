package main

import (
	"bytes"
	"encoding/json"
	"io/ioutil"
	"net"
	"sync"

	nats "github.com/nats-io/go-nats"
	log "github.com/sirupsen/logrus"
)

const (
	maxDataGramSize = 8192
)

// Config defines the config to be given for magicportal in a json file.
type Config struct {
	MulticastGroups []MulticastGroup `json:"multicast_groups"`
	Mode            string           `json:"mode"`
	NatsURL         string           `json:"nats_url"`
}

// MulticastGroup contains address and interface
type MulticastGroup struct {
	MulticastAddr string `json:"multicast_addr"`
	Interface     string `json:"interface"`
}

func main() {

	config, err := readConfig()

	if err != nil {
		log.Fatal(err)
	}

	// Connect to Nats
	nc, err := nats.Connect(config.NatsURL)

	if err != nil {
		log.Fatal("Cannot connect to NATS. Error: ", err)
	}

	log.Println("Connected to NATS.")

	addressesCount := len(config.MulticastGroups)

	var wg sync.WaitGroup

	wg.Add(addressesCount)

	// If mode is forwarder start server goroutines
	if config.Mode == "forwarder" {
		for _, grp := range config.MulticastGroups {
			go serveMulticastUDP(grp.MulticastAddr, grp.Interface, &wg, nc)
		}
	} else if config.Mode == "agent" {
		for _, grp := range config.MulticastGroups {
			connAddr, err := net.ResolveUDPAddr("udp", grp.MulticastAddr)

			if err != nil {
				log.Fatalf("Cannot resolve %v", grp.MulticastAddr)
			}

			conn, err := net.DialUDP("udp", nil, connAddr)

			nc.Subscribe(grp.MulticastAddr, func(msg *nats.Msg) {
				log.Printf("Received from NATS on %v : %v", grp.MulticastAddr, msg.Data)
				conn.Write(msg.Data)
			})
		}
	}

	wg.Wait()
}

func readConfig() (*Config, error) {
	data, err := ioutil.ReadFile("config.json")

	if err != nil {
		return nil, err
	}

	config := Config{}
	err = json.Unmarshal(data, &config)
	if err != nil {
		return nil, err
	}
	return &config, nil
}

func serveMulticastUDP(multicastAddr string, inf string, wg *sync.WaitGroup, nc *nats.Conn) {
	defer wg.Done()
	addr, err := net.ResolveUDPAddr("udp", multicastAddr)

	if err != nil {
		log.Fatalf("Got error while resolving UDP address: %v", err)
	}

	interf := net.Interface{
		Name: inf,
	}

	l, err := net.ListenMulticastUDP("udp", &interf, addr)

	if err != nil {
		log.Fatalf("Got error while listening to multicast address: %v", err)
	}

	l.SetReadBuffer(maxDataGramSize)

	log.Printf("Listening for %v", multicastAddr)

	for {
		b := make([]byte, maxDataGramSize)
		_, _, err := l.ReadFromUDP(b)
		if err != nil {
			log.Fatal("ReadFromUDP failed:", err)
		}

		message := trimBytes(b)

		// Publish to Nats
		nc.Publish(multicastAddr, message)

		log.Println("Received message: ", message)

	}
}

func trimBytes(data []byte) []byte {
	return bytes.Trim(data, "\x00")
}
