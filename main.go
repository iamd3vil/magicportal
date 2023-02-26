package main

import (
	"crypto/tls"
	"fmt"
	"net"
	"os"
	"strings"
	"sync"

	nats "github.com/nats-io/nats.go"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/pflag"
)

const (
	// DefaultMaxDataGramSize is the default maximum size of a UDP packet
	DefaultMaxDataGramSize = 1024
)

func main() {
	fset := pflag.NewFlagSet("magicportal", pflag.ExitOnError)
	fset.Usage = func() {
		fmt.Println(fset.FlagUsages())
		os.Exit(0)
	}

	cfgPath := fset.String("config", "config.toml", "Configuration path")
	fset.Parse(os.Args[1:])

	config, err := initConfig(*cfgPath)
	if err != nil {
		log.Fatalf("error initializing config: %v", err)
	}

	log.Println(config)

	var nc *nats.Conn

	if config.CfgNats.TLS {
		conf := &tls.Config{
			InsecureSkipVerify: true,
		}
		// Connect to Nats
		nc, err = nats.Connect(strings.Join(config.CfgNats.NatsURL, ","), nats.Secure(conf))
	} else {
		// Connect to Nats
		nc, err = nats.Connect(strings.Join(config.CfgNats.NatsURL, ","))
	}

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
			go func(grp CfgMulticastGroup) {
				defer wg.Done()
				serveMulticastUDP(grp.MulticastAddr, grp.Interface, nc, config)
			}(grp)
		}
	} else if config.Mode == "agent" {
		for _, grp := range config.MulticastGroups {
			var connAddr *net.UDPAddr
			var err error
			if config.CfgAgent.SendAsUnicast {
				udpAddr, ok := config.CfgAgent.UnicastAddrs[grp.MulticastAddr]
				if !ok {
					continue
				}

				connAddr, err = net.ResolveUDPAddr("udp", udpAddr)
			} else {
				connAddr, err = net.ResolveUDPAddr("udp", grp.MulticastAddr)
			}

			if err != nil {
				log.Fatalf("Cannot resolve %v", grp.MulticastAddr)
			}

			conn, err := net.DialUDP("udp", nil, connAddr)
			if err != nil {
				log.Fatalf("error dialling %s: %v", connAddr, err)
			}

			nc.Subscribe(grp.MulticastAddr, func(msg *nats.Msg) {
				conn.Write(msg.Data)
			})
		}
	}

	wg.Wait()
}

func serveMulticastUDP(multicastAddr string, inf string, nc *nats.Conn, cfg Config) {
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

	maxDataGramSize := DefaultMaxDataGramSize

	if cfg.MaxPacketSize != 0 {
		maxDataGramSize = cfg.MaxPacketSize
	}

	l.SetReadBuffer(maxDataGramSize)

	log.Printf("Listening for %v", multicastAddr)

	b := make([]byte, maxDataGramSize)
	for {
		len, _, err := l.ReadFromUDP(b)
		if err != nil {
			log.Fatal("ReadFromUDP failed:", err)
		}

		log.Printf("Number of bytes read: %d", len)

		// Publish to Nats
		nc.Publish(multicastAddr, b[:len])

		// Reset b
		b = b[:0]
	}
}
