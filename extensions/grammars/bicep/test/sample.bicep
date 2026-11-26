// Deploys a storage account, an app service and their wiring.
/* Block comments span
   several lines. */
targetScope = 'resourceGroup'

import { tagsFor } from 'shared.bicep'

type skuName = 'Standard_LRS' | 'Premium_LRS'

type appSettings = {
  name: string
  replicas: int
  owner: string?
}

@description('Prefix for every resource name.')
@minLength(3)
param prefix string = 'demo'

@secure()
param adminPassword string

@allowed([
  'dev'
  'prod'
])
param environment string = 'dev'

param location string = resourceGroup().location
param settings appSettings
param deployWeb bool = true
param containers array = ['logs', 'uploads']

var isProd = environment == 'prod'
var storageName = '${prefix}store-${uniqueString(resourceGroup().id)}'
var sku = isProd ? 'Premium_LRS' : 'Standard_LRS'
var retention = isProd ? 30 : 7
var owner = settings.?owner ?? null
var script = '''
#!/bin/sh
echo "It's ${not} interpolated"
'''

func hostName(name string) string => '${toLower(name)}.example.com'

resource storage 'Microsoft.Storage/storageAccounts@2023-01-01' = {
  name: storageName
  location: location
  kind: 'StorageV2'
  sku: {
    name: sku
  }
  properties: {
    supportsHttpsTrafficOnly: true
    minimumTlsVersion: 'TLS1_2'
    note: 'a tab\t and a quote\''
  }
}

resource blobs 'Microsoft.Storage/storageAccounts/blobServices@2023-01-01' = {
  parent: storage
  name: 'default'
}

resource folders 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-01-01' = [for name in containers: {
  parent: blobs
  name: name
}]

resource vault 'Microsoft.KeyVault/vaults@2023-07-01' existing = {
  name: '${prefix}-vault'
}

resource plan 'Microsoft.Web/serverfarms@2022-09-01' = if (deployWeb && !empty(prefix)) {
  name: '${prefix}-plan'
  location: location
  sku: {
    name: 'B1'
    capacity: isProd ? 3 : 1
  }
}

module network './modules/network.bicep' = {
  name: 'network'
  params: {
    location: location
    tags: tagsFor(environment)
    password: adminPassword
  }
}

@sys.description('Where blobs are served from.')
output endpoint string = storage.properties.primaryEndpoints.blob
output names array = [for (name, i) in containers: '${i}-${name}']
output url string = hostName(settings.name)
output days int = retention + 1
